# Nyquist — Audit consolidé exécutable (V3)

Date : 2026-08-26
Auteur du constat : audit manuel du worktree (état `main` non committé, après tag `v0.4.0`).

Ce document est destiné à une IA chargée d'appliquer les corrections. Il est **autonome** :
toutes les constatations ont été re-vérifiées sur le code actuel le 2026-08-26, avec les numéros
de ligne à jour. Il remplace `AUDIT_COMPLET.md` (V1, obsolète : citait `media_protocol.rs` qui
n'existe plus, décrivait un blocage de build désormais corrigé) et complète
`AUDIT_V2_PLAN_CORRECTION.md` (conserver pour l'historique, mais c'est ce document-ci qui fait
foi).

## Règles de travail obligatoires (AGENTS.md)

1. `git status --short` avant toute modification. Ne pas supprimer ni réinitialiser les
   changements non liés de l'utilisateur (le worktree contient déjà un travail en cours).
2. Ne pas committer ni ouvrir de PR sans demande explicite.
3. Charger la skill `.claude/skills/tauri-ipc-contract/SKILL.md` avant de modifier
   `commands.rs`, `api.ts` ou toute forme de JSON retourné au frontend.
4. Charger la skill `.claude/skills/release-packaging/SKILL.md` avant de modifier
   `tauri.conf.json`, `Cargo.toml` (versions) ou `.github/workflows/`.
5. Charger la skill `.claude/skills/dsp-correctness/SKILL.md` avant de toucher
   `signal_analysis.rs` / `spectral.rs`.
6. Charger la skill `.claude/skills/transcode-heuristic-validation/SKILL.md` avant de toucher
   `transcode_detect.rs` ou tout seuil spectral.
7. **Tout changement de `transcode_detect.rs` ou des seuils de `spectral.rs` exige de relancer
   le corpus** (`cargo test --locked --release -- --nocapture` dans `src-tauri/`) et de
   rapporter l'effet sur les faux positifs/négatifs connus avant/après.
8. Ne pas présenter la reprise automatique de lecture comme une preuve que la cause WebKit est
   corrigée (voir `.claude/INVESTIGATION-lecture-tronquee.md` — cause racine NON prouvée).
9. Docs publics et messages de commit en anglais ; fichiers `.claude/` et commentaires de code
   en français si plus naturel.
10. Citer chemins et numéros de ligne avant tout changement non trivial.

## État du dépôt et résultats des vérifications (2026-08-26)

Worktree non committé sur `main` (HEAD = `b210731`, après tag `v0.4.0`) :

```
 M .claude/CLAUDE.md              M src-tauri/src/commands.rs
 M .claude/CONTEXT.md             M src-tauri/src/lib.rs
 M CHANGELOG.md                   M src-tauri/tauri.conf.json
 M src-tauri/Cargo.lock           M src/lib/api.ts
 M src-tauri/Cargo.toml           M src/routes/+page.svelte
?? .claude/INVESTIGATION-lecture-tronquee.md
?? .claude/tools/
?? AUDIT_COMPLET.md
?? AUDIT_V2_PLAN_CORRECTION.md
?? src-tauri/src/media_server.rs
```

Le worktree remplace le protocole `asset://` par un serveur HTTP loopback
(`src-tauri/src/media_server.rs`, nouveau fichier, 679 lignes) et ajoute un mécanisme de
reprise de lecture dans `+page.svelte`. Les fichiers DSP n'ont PAS changé depuis l'audit V1.

Vérifications exécutées (état actuel du worktree) :

| Commande | Résultat |
|---|---|
| `cargo build --locked` (src-tauri) | ✅ |
| `cargo test --locked --release -- --nocapture` | ✅ 13 unit + 2 calibration + 6 corpus_smoke |
| `cargo clippy --locked --all-targets -- -D warnings` | ✅ |
| `npm run check` | ✅ 0 erreur |
| `npm run build` | ✅ |
| `cargo fmt --all -- --check` | ❌ échoue sur de nombreux fichiers préexistants (`cli/src/main.rs`, `analysis.rs`, `bit_depth.rs`, `decode.rs`, `dynamic_range.rs`, `mdct_grid.rs`, …) |

Corpus : 20 fixtures, 0 faux positif, 2 faux négatifs connus confirmés —
`transcoded_mp3_v0_44k.flac` et `transcoded_dynamic_mp3_v0_44k.flac` (tous deux classés
`ProbablyAuthentic`, confiance 0.65).

Constantes pertinentes : profils Cargo sans `overflow-checks` explicite (debug = overflow-checks
ON par défaut → l'underflow range **panique** en dev, **wrappe** en release). CI
(`.github/workflows/build.yml`) : matrice `macos-latest` + `windows-latest`.

---

# Partie 1 — Serveur média (`src-tauri/src/media_server.rs`)

## M1 — Panic au démarrage sous Windows : `/dev/urandom`

**Fichiers :** `src-tauri/src/media_server.rs:387-393` ; `src-tauri/src/lib.rs:24` ;
`.github/workflows/build.yml` (matrice windows-latest) et `release.yml`.

**Constat (vérifié) :**
- `random_token()` ouvre `File::open("/dev/urandom")` en dur avec `.expect()`.
- `lib.rs:24` fait `MediaServer::start().expect("could not bind the media server")`.
- La CI et la release buildent Windows. Sur Windows, `"/dev/urandom"` n'existe pas →
  panic au démarrage de l'application.
- Le `.expect()` transforme aussi un échec récupérable (bind refusé, etc.) en crash sans message
  utile.

**Changement demandé :**
1. Remplacer `/dev/urandom` par une source CSPRNG portable. Préféré : le crate `getrandom`
   (licence MIT/Apache-2.0, compatible licence MIT du projet — vérifier la licence dans
   `Cargo.lock` avant ajout), ou une implémentation `#[cfg(target_os)]` par plateforme. Aucun
   chemin Unix ne doit rester dans du code non conditionné.
2. `lib.rs:24` : ne pas faire paniquer l'application au démarrage. Si `MediaServer::start()`
   échoue, retourner une erreur exploitable (propager depuis le setup, ou logger + état
   « lecture indisponible ») plutôt que `.expect()`.
3. La chaîne d'erreur de bind doit être explicite (port refusé, etc.).

**Tests :**
- Test unitaire : deux appels à `random_token()` produisent des valeurs différentes.
- Le serveur démarre sur Windows CI (`cargo build` + un test qui appelle `MediaServer::start()`).

## M2 — Underflow u64 sur range inversé (`bytes=100-50`)

**Fichiers :** `src-tauri/src/media_server.rs:414-425` (`parse_range`), `255-275` (`serve_request`).

**Constat (vérifié) :**
- `parse_range("bytes=100-50")` retourne `Some((100, Some(50)))`.
- Dans `serve_request`, branche `Some((start, end))` (l.261-263) :
  `end = end.unwrap_or(len - 1).min(len - 1); let count = end - start + 1;`
  → `50 - 100 + 1` sous-déborde en u64 : **panic en dev** (overflow-checks par défaut),
  **count énorme en release** (réponse `Content-Length` absurde, `copy_exact` se termine par
  EOF → connexion coupée au milieu d'une réponse invalide).
- Le guard existant (`start >= len` → 416, l.256) ne couvre pas `end < start`.
- `parse_range` confond aussi une range malformée avec une range ouverte : `bytes=0-abc` →
  `Some((0, None))` (le `.parse().ok()` de la fin avale l'erreur, l.424).

**Changement demandé :**
1. Faire échouer le parsing ou le serveur proprement pour `end < start` :
   - soit `parse_range` retourne un type à trois états `Absent | Valid(ByteRange) | Invalid`,
   - soit `serve_request` teste `end < start` et répond `416 Range Not Satisfiable`.
2. `bytes=0-abc` doit être rejeté (416) ou retourner `Invalid` — pas une range ouverte.
3. `bytes=-500` (suffixe) : garder le comportement actuel documenté (réponse complète, pas
   d'erreur) OU rejeter par 416 — mais la décision doit être explicite et testée. Ne pas
   calculer une longueur négative dans tous les cas.
4. Vérifier la cohérence `Content-Range` / `Content-Length` sur chaque branche.

**Tests (socket réels, dans le module) :**
- `0-0`, `len-1-len-1`, `start >= len`, range ouverte `bytes=N-`, suffixe `bytes=-N`,
  inversée `bytes=100-50`, multi-range, caractères invalides, fichier vide, `HEAD` avec et
  sans range. Chaque réponse doit avoir un statut et des headers cohérents.

## M3 — Surface de déni de service : thread par connexion, buffers non bornés, pas de write timeout

**Fichiers :** `src-tauri/src/media_server.rs:166-177` (accept loop + thread par connexion),
`327-376` (`read_request` : `read_line` sans borne sur request-line ET sur chaque header),
`194-204` (`serve_connection` : read timeout 30 s mais pas de write timeout).

**Constat (vérifié) :** un processus local peut ouvrir des centaines de sockets (thread par
connexion, aucun pool borné) ou envoyer une request-line / des headers infinis (l'allocation
`String` croît sans limite). Le token n'est vérifié qu'après la lecture de la request-line.
Un client qui cesse de lire bloque une écriture `write_all` sans timeout.

**Changement demandé :**
1. Limite de connexions simultanées (sémaphore ou pool de workers borné, par ex. 32) ; au-delà,
   refuser proprement.
2. Taille maximale de la request-line (par ex. 8 KiB) et des headers (par ex. 8 KiB chacun,
   total 64 KiB) ; rejeter `431` ou fermer proprement.
3. Write timeout sur `TcpStream` (par ex. 30 s), en gardant le read timeout existant.
4. Politique documentée : un client qui ne lit pas → connexion coupée après timeout, pas de
   fuite de thread.
5. Aucun buffer contrôlable par un client ne doit être non borné.

**Tests :** connexions inactives simultanées au-delà de la limite, request-line de 1 MiB,
headers très longs, client qui n'accuse jamais réception, récupération du serveur après rejet.

## M4 — TOCTOU symlink : autorisation puis ouverture sur deux chemins différents

**Fichiers :** `src-tauri/src/media_server.rs:110-118` (`PlaybackScope::allow`/`is_allowed`,
canonicalise le chemin), `124-126` (`canonical`), `231-234` (`File::open(&path)` sur le chemin
brut décodé de l'URL).

**Constat (vérifié) :** le chemin est canonicalisé pour le contrôle d'allowlist, puis
`File::open` rouvre le chemin brut. Un remplacement de symlink entre les deux opérations (ou
entre l'autorisation et la requête) peut faire servir un fichier non autorisé.

**Changement demandé :** ouvrir le fichier au moment de l'autorisation et servir une ressource
stable, plutôt que de rouvrir par chemin :
1. `authorize()` : ouvrir le fichier, conserver (a) le handle ou (b) une identité vérifiée
   (dev/inode via `MetadataExt` sur Unix, équivalent Windows), et associer un identifiant
   opaque (nonce) à cette ressource.
2. L'URL ne devrait plus contenir le chemin filesystem brut ; servir par identifiant opaque.
3. Si la réouverture par chemin est conservée (choix plus simple), vérifier l'identité du
   fichier ouvert contre celle enregistrée, et refuser en cas de divergence. Documenter que la
   simple canonicalisation n'est pas atomique.
4. Compatible avec la vue comparaison : plusieurs ressources autorisées simultanément (le
   scope est déjà additif, le préserver).

**Tests :** remplacement de symlink après autorisation, remplacement du fichier pendant une
lecture, chemin supprimé, plusieurs ressources autorisées en parallèle.

## M5 — MIME choisi par extension, pas par le média réellement analysé

**Fichiers :** `src-tauri/src/media_server.rs:128-146` (`mime_for`) ; le commentaire l.132
affirme « the set below is exactly the set `analyze_file` accepts » — faux : le probe Symphonia
fonctionne par contenu ; `src/routes/+page.svelte:149,168` (sélecteur de fichiers sans `.opus`).

**Constat (vérifié) :**
- `.opus` absent de `mime_for` → servi `application/octet-stream` (WebKit refuse).
- Extension fausse ou absente → MIME faux alors que l'analyse aurait réussi (probe par contenu).
- `symphonia` sonde par contenu : l'ensemble des fichiers analysables n'est pas l'ensemble des
  extensions listées.

**Changement demandé :**
1. Choisir une seule source de vérité :
   - (recommandé) passer le MIME déterminé par l'analyse (conteneur/codec Symphonia) à
     l'autorisation — modifier `commands.rs:38-39` (`authorize_playback`) et `api.ts` en
     cohérence (charger la skill `tauri-ipc-contract`) ; ou
   - détecter le format au moment de l'ouverture par les premiers octets avec une table validée
     (FLAC fLaC, MP3 frames, RIFF, ftyp/MP4, OggS, OpusHead…).
2. Ajouter `.opus` (et `.oga`) à `mime_for` ET au sélecteur frontend
   (`+page.svelte:149,168`).
3. Supprimer ou corriger le commentaire « exactly the set analyze_file accepts ».
4. Couvrir au minimum FLAC, MP3, WAV, M4A/ALAC, AAC brut, OGG, Opus ; extensions en majuscules,
   absentes, incorrectes.

**Tests :** MIME correct pour chaque extension supportée (majuscules incluses), MIME correct
quand l'extension est absente ou fausse, `.opus` servi `audio/ogg` (ou MIME déterminé par le
codec).

## M6 — Cache : URL stable par chemin, contenu périmé possible

**Fichiers :** `src-tauri/src/media_server.rs:187-190` (`authorize` : URL identique pour un même
chemin pendant toute la session), `239-245` (`Cache-Control: private, max-age=3600`),
`289-297` (`entity_tag` = len + mtime en secondes). `read_request` ne parse pas
`If-None-Match` (aucune revalidation possible).

**Constat (vérifié) :** si le fichier est remplacé au même chemin, l'analyse porte sur le
nouveau contenu tandis que WebKit peut lire l'ancienne réponse pendant 1 h (ETag taille+mtime
à la seconde + `max-age` = pas de revalidation fiable).

**Changement demandé :**
1. (recommandé, en lien avec M4) URL opaque nouvelle pour chaque autorisation
   (`/{token}/{nonce}`) pointant vers une ressource immuable → `Cache-Control` long possible,
   aucune ambiguïté.
2. Ou : gérer `If-None-Match` → `304 Not Modified`, ETag sans `max-age` long
   (`no-cache`/`must-revalidate`), et tests de remplacement du fichier.

**Tests :** remplacement du fichier au même chemin puis nouvelle analyse → la lecture sert le
nouveau contenu ; réanalyse immédiate ; le cache ne renvoie jamais d'ancien contenu.

## M7 — Déjà corrigé depuis V1 (ne pas refaire)

- GET/HEAD sans Range : le serveur stream désormais par `copy_exact` (l.303-323, buffer 64 KiB)
  au lieu de `read_to_end` — V1 citait `media_protocol.rs` qui n'existe plus.
- HEAD (l.246, 267-268, 278-280) : réponse headers-only correcte, sans corps.
- Le mismatch `protocol-asset` (V1) est résolu : la feature Cargo a été retirée, `cargo build`
  et `cargo test` passent.

---

# Partie 2 — Verdict de transcodage et DSP (préexistant, non modifié depuis V1)

## T1 — P0 : absence d'indice → `ProbablyAuthentic` (faux négatifs LAME V0 confirmés)

**Fichiers :** `src-tauri/src/transcode_detect.rs:505-541` ; corpus `src-tauri/tests/fixtures/corpus/`.

**Constat (vérifié + corpus) :** quand `spectral.encoder_edge_hz` est `None`, le code construit
des indicateurs « aucune coupure » + « encodages transparents » puis retourne
`Verdict::ProbablyAuthentic` avec une confiance de base `NO_EDGE_CONFIDENCE` (0.60-0.70 ; 0.65
observé). Le corpus contient DEUX transcodages MP3 LAME V0 réels classés `ProbablyAuthentic`
0.65 : `transcoded_mp3_v0_44k.flac` et `transcoded_dynamic_mp3_v0_44k.flac`. L'application
cautionne donc activement un faux lossless dans le cas le plus important du point aveugle MP3.
Le bonus « bandwidth above CD ceiling » (l.525-533) est une preuve POSITIVE légitime — le
conserver comme bonus, pas comme base.

**Changement demandé :**
1. Par défaut, sans preuve positive, retourner `Verdict::Indeterminate` (nouveau verdict
   d'absence de preuve), en gardant les indicateurs `NoEncoderLowpass` et
   `TransparentEncodeUnseen` (ou renommer pour dire « aucun indice détecté »).
2. Réserver `ProbablyAuthentic` à une combinaison de preuves positives et indépendantes
   (ex. contenu au-dessus du plafond CD + pas d'edge, éventuellement + tags sans conflit).
3. Si un verdict `ProbablyAuthentic` est conservé dans certains cas, la confiance doit
   refléter la force des preuves positives, jamais une base par défaut.
4. Ne pas toucher aux seuils spectraux dans cette correction (uniquement le verdict/la logique
   de décision).
5. Mettre à jour les textes i18n et le README pour « aucun indice de transcodage détecté »
   au lieu de « authentique ».

**Validation obligatoire :** relancer le corpus (`cargo test --locked --release -- --nocapture`)
et rapporter avant/après. Attendu : 0 faux positif conservé, les deux LAME V0 passent à
`Indeterminate` (plus aucun `ProbablyAuthentic` sur un fichier non authentique).

## T2 — P0 : verdict calculé sur un décodage incomplet

**Fichiers :** `src-tauri/src/decode.rs:89-110` ; `src-tauri/src/analysis.rs:148-156` ;
`src-tauri/src/metadata.rs:32,66` (`decode_errors` dans `FileInfo`).

**Constat (vérifié) :**
- `IoError`/`DecodeError` → `decode_errors += 1` puis `continue` (le paquet est sauté, l'analyse
  continue).
- `ResetRequired` (OGG chaîné) → `break` **sans** incrémenter `decode_errors` et sans marquer le
  décodage comme incomplet.
- `analysis.rs:150-156` appelle `assess_transcode_risk(&spectral_analysis, nyquist, tags,
  mdct_grid, codec_short_name)` — **sans** `decode_errors` ni statut de complétude. Le verdict
  est donc produit normalement sur un signal partiel.
- `decode_errors` est exposé dans `FileInfo` (affiché par le CLI `cli/src/main.rs:107-111` et
  par l'UI) mais n'influence aucun verdict.

**Changement demandé :**
1. Ajouter au contrat un état explicite de complétude, par ex. `decode_status` /
   `audio_coverage` (voir IPC-F2), calculé dans `decode.rs` : `ResetRequired` doit marquer le
   décodage incomplet ; toute erreur sautée aussi.
2. `assess_transcode_risk` (ou `analysis.rs`) : si le décodage est incomplet, forcer le verdict
   à `Indeterminate` (ou `DecodedIncomplete`, état distinct à discuter) et une confiance très
   basse, avec un indicateur explicite « décodage incomplet ».
3. Conserver les mesures disponibles, mais les présenter comme mesures d'un extrait survivant
   (UI + CLI).
4. Vérifier la cohérence longueur déclarée vs échantillons réellement analysés.

**Tests (à ajouter) :**
- FLAC tronqué (avec et sans MD5 valide dans le header) → verdict non affirmatif.
- Fichier avec un paquet illisible puis des paquets décodables → mesures partielles marquées.
- OGG chaîné provoquant `ResetRequired` → `decode_status` incomplet.
- Assertion que longueur déclarée et longueur analysée sont cohérentes ou signalées.

## T3 — P1 : bande passante repliée sur Nyquist quand rien n'est mesuré

**Fichiers :** `src-tauri/src/spectral.rs:276-280` ; `src-tauri/src/sample_rate.rs:63-88` ;
`src/lib/api.ts:90-106` ; `src/routes/+page.svelte:451-462`.

**Constat (vérifié) :** quand `find_spectral_edge` (sonde large) ne trouve aucune limite,
`spectral_cutoff_hz = nyquist_hz` (l.279-280). Un fichier bass-only est donc affiché comme
ayant 22,05 kHz de bande passante à 100 % (observé dans `authentic_bass_only_44k`), et
`sample_rate.rs` prend cette valeur de repli pour une mesure réelle.

**Changement demandé :**
1. Remplacer `spectral_cutoff_hz: f64` par `Option<f64>` (ou ajouter
   `spectral_cutoff_measured: bool` + champ séparé pour la valeur de repli). Implique une mise
   à jour du contrat IPC (`api.ts`) — charger `tauri-ipc-contract`.
2. Distinguer trois notions : limite de contenu observée, edge de filtre détectée, Nyquist
   déclaré.
3. `analyze_sample_rate` doit ignorer une mesure non établie (pas de ratio 100 % fabriqué ;
   pas de conclusion `likely_upsampled` sur un repli).
4. UI : afficher « aucune limite mesurable » / `n/d` au lieu d'un chiffre de Nyquist ; textes
   de la fiche technique à mettre à jour en conséquence.

**Tests :** fixture bass-only → `spectral_cutoff_hz` = `None` ; la fiche ne prétend plus une
bande passante ; pas de faux `likely_upsampled` déclenché par le repli.

## T4 — P1 : meilleur candidat spectral choisi avant les gates

**Fichiers :** `src-tauri/src/spectral.rs:557-588` (`find_spectral_edge`).

**Constat (vérifié) :** le balayage conserve uniquement la chute la plus forte (`best_hz`,
l.559-573), puis applique les gates (occupation de bande l.576-580, profondeur du stopband
l.582-586). Si ce candidat unique est un notch musical rejeté par les gates, le second
candidat — qui pourrait être la vraie coupure codec — n'est jamais testé. Faux négatifs
possibles sur de la musique réelle avec plusieurs accidents spectraux.

**Changement demandé :**
1. Conserver les N meilleurs candidats par chute (top 5), triés.
2. Appliquer les gates à chacun dans l'ordre ; retourner le premier candidat valide.
3. (optionnel) Exposer un score de séparation entre le candidat retenu et les suivants.

**Test :** spectre synthétique avec un notch profond rejeté par les gates puis une coupure
codec moins profonde mais valide → la coupure doit être sélectionnée.

## T5 — P1 : `cutoff_over_time_hz` calculé mais absent du verdict

**Fichiers :** `src-tauri/src/spectral.rs:136-153,282-286` ; `src-tauri/src/transcode_detect.rs:486-513`.

**Constat (vérifié) :** le spectrogramme temporel et `cutoff_over_time_hz` sont calculés et
affichés, mais le scoring de transcodage n'utilise que le spectre moyen global. Un transcodage
présent sur une portion du fichier peut être dilué par le reste.

**Changement demandé (choix à faire) :**
1. **Option A (recommandée)** : calculer un edge par fenêtre active, avec référence locale et
   globale, et scorer la proportion de fenêtres montrant un edge cohérent
   (`edge_presence_ratio`, `active_window_count`, minimum d'observations). C'est un vrai gain
   de détection mais c'est un changement de scoring → obligatoirement validé sur le corpus
   avec rapport FP/FN avant/après.
2. **Option B (minimale)** : si la donnée reste descriptive, retirer de la documentation toute
   promesse de détection locale (« détecte un transcodage présent sur une partie du fichier »)
   et laisser le scoring inchangé.

## T6 — P1 : confiance affichée comme pourcentage non calibré

**Fichiers :** `src-tauri/src/transcode_detect.rs:67-124` (constantes de confiance) ;
`src/routes/+page.svelte:411-415` (affichage `%`) ; corpus README.

**Constat (vérifié) :** les scores (0.25, 0.60, 0.65, 0.75, 0.80, 0.90, 0.95…) sont des valeurs
heuristiques réglées sur un petit corpus, pas des probabilités. L'UI les affiche comme des
pourcentages (« 90 % »), donnant une précision apparente non justifiée. Corpus majoritairement
synthétique, bruit stationnaire, un seul encodeur AAC dominant.

**Changement demandé :**
1. Remplacer l'affichage « confiance N % » par une notion de **force des indices**
   (faible / modérée / forte), tout en conservant le score brut dans le JSON technique.
2. Ne pas afficher le symbole `%` tant qu'il n'existe pas de calibration sur un corpus tenu à
   l'écart (corpus de réglage vs corpus de validation).
3. Documenter la composition du corpus dans le README du corpus et rapporter précision/rappel/
   FP/FN sur le corpus de validation.

## T7 — P1 : grille MDCT AAC plus limitée que son libellé

**Fichiers :** `src-tauri/src/mdct_grid.rs:24-43,51-74,126-186` ; `src/lib/i18n.svelte.ts:288-291`.

**Constat (vérifié) :** l'algorithme suppose MDCT AAC longue (1024 échantillons), fenêtre sine,
et n'analyse que le premier canal. Short blocks, block switching, autres fenêtres (KBD),
layouts multicanaux ou premier canal silencieux (avec le signal dans un autre canal) → résultat
inexploitable ou faux. Les textes présentent pourtant la détection comme couvrant « l'AAC ».

**Changement demandé :**
1. Choisir le canal actif le plus informatif (max d'énergie) au lieu du premier canal.
2. Retourner le niveau d'énergie et le nombre de canaux examinés (pour honnêteté du rapport).
3. Limiter les textes à « AAC long-block compatible » tant que le périmètre n'est pas élargi.
4. Ajouter un état `not_applicable`/`unsupported_profile` distinct de `clear`.

**Tests :** AAC avec transitoires (short blocks), plusieurs encodeurs AAC, AAC ré-échantillonné
puis converti en lossless, premier canal silencieux / second canal actif, fenêtre KBD.

## T8 — P1 : limite basse fixe de 8 kHz

**Fichiers :** `src-tauri/src/spectral.rs:73-79` (`MIN_PLAUSIBLE_ENCODER_CUTOFF_HZ`).

**Constat (vérifié) :** les coupures sous 8 kHz sont ignorées comme « contenu naturellement
étroit ». Certains encodages à très bas débit deviennent invisibles.

**Changement demandé :** ne pas baisser le seuil brutalement. Soit ajouter un classifieur de
contexte (occupation broadband, durée de l'edge, cohérence temporelle) validé sur le corpus,
soit documenter la limite et garantir que le résultat reste `Indeterminate` dans ce cas.

## T9 — P2 : faux hi-res sur masters naturellement sombres

**Fichiers :** `src-tauri/src/sample_rate.rs:33-44,63-88` ; `src-tauri/tests/corpus_smoke.rs:493-537`.

**Constat (vérifié) :** un master authentique 96/192 kHz naturellement limité dans les aigus
peut franchir `MIN_BANDWIDTH_RATIO` et être marqué `likely_upsampled`. Pas de fixture hi-res
naturellement sombre dans le corpus. La tolérance 0.9 (`BANDWIDTH_TOLERANCE`) appliquée pour
`sufficient_sample_rate_hz` n'est pas évidente pour l'utilisateur. De plus, `spectral_cutoff_hz`
y entre brut (cf. T3) : le repli Nyquist d'un fichier sombre peut faire passer la mesure pour
une bande complète.

**Changement demandé :**
1. Ajouter des fixtures authentiques hi-res à bande naturellement limitée.
2. Ne marquer `likely_upsampled` que sur une bande réellement mesurée (après T3).
3. Afficher « compatible avec un sur-échantillonnage » plutôt que « fréquence gonflée ».
4. Documenter la tolérance appliquée.
5. Ne jamais faire monter la confiance d'authenticité sur cette heuristique seule.

## T10 — P1 : tags incomplets ou comptés de travers

**Fichiers :** `src-tauri/src/tags.rs:41-70` (`scan_for_lossy_encoder_traces`) ;
`src-tauri/src/transcode_detect.rs:448-474` (`apply_tag_evidence`).

**Constat (vérifié) :**
- Seuls les tags disponibles immédiatement après le probe sont lus (commenté : pas
  d'APEv2/ID3v1 en fin de fichier).
- Un même tag contenant deux motifs produit deux `EncoderTagMatch` (boucle sur les patterns,
  l.60-67).
- `additional_matches: matches.len() - 1` (l.458) est présenté comme un nombre de tags
  supplémentaires alors que ce sont des patterns supplémentaires dans le même tag.

**Changement demandé :**
1. Dédupliquer par couple `(tag_key, tag_value)` et/ou distinguer `matching_patterns` de
   `matching_tags` dans le contrat (`EncoderTagMatch`, `api.ts`).
2. Documenter (UI et code) que les tags de fin de fichier (ID3v1/APEv2) ne sont pas couverts.
3. L'absence de tag reste une absence d'information — jamais une preuve d'authenticité (déjà
   le cas, le documenter dans l'UI).

**Tests :** plusieurs motifs dans un même tag, plusieurs tags, tags de fin de fichier.

## T11 — P2 : verdict `DeclaredLossy` incohérent avec la documentation (3 vs 4 états)

**Fichiers :** `src-tauri/src/transcode_detect.rs:147-163,353-363` (4 états dans le code) ;
`README.md:53-57` (« trois états ») ; `src-tauri/cli/src/main.rs:187` (affiche `100 %` pour
`DeclaredLossy`, confiance 1.0) ; `src/lib/api.ts:110-117`.

**Changement demandé :**
1. Documenter explicitement les quatre états dans le README et le doc-comment de `Verdict`
   (déjà bien fait dans le code, l.147-162).
2. Remplacer la confiance `1.0` de `DeclaredLossy` par `null` ou introduire
   `confidence_kind: declared | inferred` (voir IPC-F2).
3. CLI et UI doivent afficher la même sémantique (l'UI masque déjà le % pour `declared_lossy` ;
   aligner le CLI).
4. Ajouter un golden JSON par état.

## T12 — P2 : liste de codecs « déclarés lossy » non exhaustive

**Fichiers :** `src-tauri/src/transcode_detect.rs:165-174` (`is_declared_lossy` : mp1/mp2/mp3,
aac, vorbis, opus).

**Constat (vérifié) :** la liste est explicite et sûre dans le sens « codec inconnu → on
conserve l'analyse » (documenté l.171-173) — c'est le comportement sûr. Mais un codec lossy que
Symphonia sait décoder et qui manque à la liste produira un verdict de « secret » sur un fichier
qui ne cache rien.

**Changement demandé :** lister les codecs réellement décodables par Symphonia dans le périmètre
supporté, documenter ce périmètre dans le README, et ajouter un test par codec supporté. Ne
jamais conclure qu'un codec inconnu est lossless.

---

# Partie 3 — Justesse des mesures DSP (préexistant)

## D1 — P1 : LUFS potentiellement faux en multicanal (pas de layout)

**Fichiers :** `src-tauri/src/decode.rs:18-25` (`DecodedAudio` : `channels` sans layout) ;
`src-tauri/src/signal_analysis.rs:163-175` (passage du nombre de canaux à `ebur128::new`).

**Constat (vérifié) :** `ebur128` reçoit seulement le nombre de canaux et applique sa carte par
défaut. En 5.1/7.1, la position des canaux (LFE, centre, surrounds) compte dans BS.1770
(poids des canaux, pas d'énergie sur le LFE). Une énergie mal placée donne un LUFS différent
de la référence.

**Changement demandé :**
1. Conserver le layout des canaux (`AudioSpec::channels()` de Symphonia) dans `DecodedAudio`
   (représentation sérialisable).
2. Mapper explicitement les positions Symphonia vers `ebur128::Channel`.
3. Layout inconnu ou mapping non fiable → état « layout inconnu » exposé dans le rapport, pas
   de valeur silencieusement fausse.
4. Pas de downmix générique avant LUFS.

**Tests :** 3.0, 5.0, 5.1, 7.1 ; signaux isolés dans le LFE/centre/surrounds ; comparaison avec
libebur128 ou ffmpeg.

## D2 — P1 : seuil de clipping fixé pour du 16 bits

**Fichiers :** `src-tauri/src/signal_analysis.rs:14-26` (`CLIPPING_THRESHOLD = 1 - 1/32768`) ;
`79-86` (comptage `s.abs() >= CLIPPING_THRESHOLD`).

**Constat (vérifié) :** le seuil est toujours le même quelle que soit la profondeur. Pour du
24 bits, des échantillons valides dans les ~65536 derniers codes sous le plein échelle sont
comptés comme écrêtés. Le commentaire « stays far enough above normal peaks » est vrai en
16 bits, faux en 24 bits. Un échantillon proche du plein échelle n'implique pas une waveform
aplatie.

**Changement demandé :**
1. Adapter le seuil à la profondeur connue (via `bits_per_sample` quand il est fiable).
2. Codec sans profondeur PCM fiable → état de mesure distinct.
3. Séparer `full_scale_sample_count` (échantillons ≥ seuil) de `clipping_count` (preuve de
   saturation : répétitions, plateaux, suites de samples au plafond).
4. Nuancer les textes i18n (« forme d'onde aplatie » → « compatible avec un écrêtage »).

**Tests :** rampes 16 et 24 bits juste sous le plein échelle ; square wave valide ; valeurs
proches du plein échelle sans écrêtage ; signaux flottants.

## D3 — P1 : true peak non homogène à 192 kHz + erreur avalée en `-120 dBTP`

**Fichiers :** `src-tauri/src/signal_analysis.rs:148-197` ; `src-tauri/Cargo.toml:36-39`.

**Constat (vérifié) :**
- `ebur128` (documentation locale) : oversampling 4x sous 96 kHz, 2x entre 96 et 192 kHz,
  aucun à 192 kHz. Le code présente toujours le résultat comme un true peak.
- `true_peak(ch).unwrap_or(0.0)` (l.190) : une erreur de la bibliothèque devient `0.0` →
  affiché `-120 dBTP` (via `linear_to_db`), un résultat plausible mais faux.

**Changement demandé :**
1. Ajouter `oversampling_factor` au résultat (0, 2 ou 4).
2. À 192 kHz (ou quand le facteur est < 4), libeller « peak échantillonné » et documenter la
   limite dans l'UI.
3. Propager l'erreur (`?` / `Err`) au lieu de `unwrap_or(0.0)`.
4. (optionnel, à discuter) Tester la feature `precision-true-peak` d'ebur128 après benchmark.

**Tests :** signaux intersample connus à 44.1, 96 et 192 kHz contre une référence.

## D4 — P1 : détection de bit depth influencée par les silences

**Fichiers :** `src-tauri/src/bit_depth.rs:85-108`.

**Constat (vérifié) :** les zéros sont « alignés sur toutes les grilles » (l.93-96 : `tz = 63`
pour les zéros) et comptent dans `total_samples`. Avec un seuil d'alignement de 99,9 %
(`ALIGNMENT_THRESHOLD`), un vrai 24 bits contenant un très long silence est signalé comme
16 bits ou moins.

**Changement demandé :**
1. Calculer l'alignement sur les échantillons actifs uniquement (hors zéros), avec un seuil et
   une couverture documentés.
2. Retourner le taux d'échantillons actifs et la couverture de l'observation.
3. Refuser le résultat (ou `None`) sur un fichier entièrement ou presque silencieux.
4. Assumer et afficher le faux négatif du dither comme limite connue.

**Tests :** 24 bits avec 99,9 % de silence ; impulsion 24 bits non alignée sur 16 ; silence
pur ; 16→24 avec dither.

## D5 — P2 : plancher `-120 dB` qui masque la nature de la mesure

**Fichiers :** `src-tauri/src/signal_analysis.rs:10-33` (`SILENCE_FLOOR_DB`).

**Constat (vérifié) :** valeurs nulles, très faibles et « non mesurables » sont toutes
affichées `-120 dB` (peak, RMS, true peak, crest factor).

**Changement demandé :**
1. Retourner `null` pour une valeur non mesurable, avec un champ
   `is_floor_value`/`measurement_status` si le frontend doit l'afficher.
2. Afficher `n/d` pour un crest factor ou un peak sans sens sur le silence.
3. Conserver éventuellement une valeur de plancher séparée pour l'affichage graphique.

## D6 — P2 : LRA de fichiers courts affichée comme zéro valide

**Fichiers :** `src-tauri/src/signal_analysis.rs:184-190`.

**Constat (vérifié) :** `loudness_range()` peut retourner un `0.0` fini quand l'historique
short-term est vide ou trop court ; le filtre `lufs.is_finite()` le laisse passer → LRA 0
sérialisée comme réelle.

**Changement demandé :**
1. Définir une durée minimale et un nombre minimal de blocs short-term conforme à
   EBU Tech 3342 (min ~5 s utiles).
2. Retourner `None` en dessous, et afficher une note « durée insuffisante » dans l'UI.

**Tests :** signaux de 1 s, 2,9 s, 5 s, 10 s ; silence ; deux niveaux distincts sur courte durée.

## D7 — P2 : `effectively_mono` inatteignable

**Fichiers :** `src-tauri/src/stereo.rs:94-101` (test `< SIDE_NEGLIGIBLE_DB`) et `134-144`
(`energy_ratio_db` plafonne à `.max(SIDE_NEGLIGIBLE_DB)`) ; `SIDE_NEGLIGIBLE_DB = -60.0` (l.25).

**Constat (vérifié) :** `energy_ratio_db` ne peut jamais retourner moins de -60 dB, donc
`side_to_mid_db < -60` est toujours faux → `effectively_mono` est toujours `false`.

**Changement demandé :**
1. Garder la valeur mesurée (brute) séparée de la valeur d'affichage (plafonnée).
2. Tester `<= SIDE_NEGLIGIBLE_DB` sur la valeur brute, ou utiliser un seuil strictement
   supérieur au plancher d'affichage.
3. Ajouter un test : deux canaux presque identiques mais pas bit-identiques → `effectively_mono`
   = true.

## D8 — P2 : longueurs de canaux incohérentes

**Fichiers :** `src-tauri/src/decode.rs:112-126` ; `src-tauri/src/metadata.rs:40-64` ;
`src-tauri/src/spectral.rs:190-204` ; `src-tauri/src/stereo.rs:72-78`.

**Constat (vérifié) :** la durée et le nombre d'échantillons prennent le premier canal ;
le spectre et la stéréo prennent le plus court (`min`). Un canal tronqué produit des sections
du rapport parlant de durées différentes.

**Changement demandé :** imposer des longueurs de canaux identiques au décodage (rejeter ou
tronquer explicitement en marquant le rapport dégradé), ou exposer
`min_sample_count`/`max_sample_count` et utiliser la longueur commune uniquement après avoir
marqué le rapport comme dégradé.

## D9 — P2 : hypothèses fortes sur le contenu dans le spectre

**Fichiers :** `src-tauri/src/spectral.rs:34-45,427-475,496-518`.

**Constat (vérifié) :** le cutoff peak-relative, l'occupation de bande et les moyennes restent
sensibles au genre, aux transitoires et aux passages calmes. Le scoring peut donner une
confiance injustifiée sur du matériau dont la qualité ne permet pas la mesure.

**Changement demandé :**
1. Conserver des quantiles temporels plutôt qu'une seule moyenne globale.
2. Mesurer séparément occupation broadband et pente.
3. Ajouter un score de « qualité du matériau » (énergie HF suffisante, nombre de fenêtres
   actives, largeur de bande utile) et ne pas faire monter la confiance quand il est faible.
4. (le plus simple en attendant) Documenter la limite dans l'UI.

---

# Partie 4 — Frontend (`src/routes/+page.svelte`, `src/lib/`)

Toutes les lignes citées ont été re-vérifiées le 2026-08-26. Pour les changements de contrat
(`api.ts`), charger `tauri-ipc-contract`.

## F1 — P1 : courses entre analyses concurrentes (pas de token de génération)

**Fichier :** `src/routes/+page.svelte:122-142` (`analyze()`).

**Constat (vérifié) :** aucune génération/requestId nulle part (`rg token|requestId|generation`
: 0 résultat). Deux drops/sélections rapides : l'ancienne Promise écrit `result`,
`audioSrc`, `error` ou `loading` après la nouvelle. Le `finally` n'est pas conditionné.

**Changement demandé :**
1. Ajouter un compteur de génération monotone (module-level).
2. Capturer la génération dans `analyze()` ; chaque continuation (résultat, catch, finally,
   lecture, reprise) vérifie `generation === currentGeneration` avant d'écrire l'état.
3. Les callbacks du lecteur (`timeupdate`, `ended`, `loadedmetadata`, `onerror`) doivent
   vérifier la même génération avant de toucher l'UI.

**Tests :** deux analyses avec réponses inversées ; analyse rapide puis lente ; autorisation
OK mais analyse échouée.

## F2 — P1 : `Promise.all([analyzeFile, authorizePlayback])` masque le rapport

**Fichier :** `src/routes/+page.svelte:134-138`.

**Constat (vérifié) :** si `authorizePlayback` échoue (lecture indisponible), le catch global
met `error` et le rapport d'analyse réussi n'est jamais affiché.

**Changement demandé :** afficher le rapport dès que `analyzeFile` réussit ; stocker
`playbackError` séparément ; rendre un lecteur désactivé avec explication.

## F3 — P1 : `play()` sans catch + erreurs audio silencieuses

**Fichier :** `src/routes/+page.svelte:177` (`togglePlay`), `213` (`el.play()` dans la reprise
`loadedmetadata`), `810` (`onerror={refreshDiag}`).

**Constat (vérifié) :** les rejets de `play()` (autoplay policy, format non supporté) sont
non capturés. `onerror` ne met à jour que le diagnostic temporaire. Aucun état `playbackError`.

**Changement demandé :**
1. `try/catch` (ou `.catch()`) sur chaque `play()`, y compris la reprise.
2. État `playbackError` + traduction i18n ; distinguer « analyse réussie » de « lecture
   indisponible ».
3. Gérer au minimum `error`, `stalled`, `waiting` sans les confondre avec une fin naturelle.
4. (optionnel) État `canplay` avant d'activer le bouton lecture.

## F4 — P2 : ancienne lecture non explicitement arrêtée

**Fichier :** `src/routes/+page.svelte:128` (`audioSrc = null` dans `analyze()`).

**Constat (vérifié) :** aucun `pause()` / `currentTime = 0` / `load()` sur l'ancien élément ;
l'arrêt repose sur le retrait DOM du `{#if audioSrc}`. Un ancien élément peut encore émettre
des événements (ex. `ended` de l'ancien fichier déclenche la reprise sur le nouveau).

**Changement demandé :** avant de remplacer la source, mettre en pause l'élément actuel,
réinitialiser `currentTime`, réinitialiser les états (`isPlaying`, `playbackError`), puis
démonter. Les listeners `ended`/`loadedmetadata` de la reprise doivent vérifier la génération.

## F5 — P2 : erreurs de comparaison invisibles / erreurs principales dupliquées

**Fichier :** `src/routes/+page.svelte:158` (catch de `pickAndCompare`), `405` et `419`
(rendus `{#if error}`).

**Constat (vérifié) :** l'erreur de comparaison va dans `error` partagé ; quand `result`
existe, aucun bloc ne l'affiche. Deux emplacements affichent `error`, risque de doublon.

**Changement demandé :** ajouter `compareError` distinct, l'afficher près de la vue de
comparaison, centraliser le rendu de l'erreur principale.

## F6 — P2 : export sans gestion d'exception ni confirmation

**Fichier :** `src/routes/+page.svelte:243-251` (`handleExport`).

**Constat (vérifié) :** `open()`, `save()`, `exportReport()` sans try/catch ni message de
succès.

**Changement demandé :** try/catch sur chaque dialogue et l'IPC ; message de succès temporaire ;
afficher le chemin choisi si compatible UX.

## F7 — P2 : mute sans mémorisation du volume précédent

**Fichier :** `src/routes/+page.svelte:229-231` (`toggleMute`), `775` (label).

**Constat (vérifié) :** `muted = !muted` uniquement. À `volume === 0`, le bouton dit
« unmute » mais ne restaure rien (l.775 : `muted || volume === 0 ? unmute : mute`).

**Changement demandé :** mémoriser `lastAudibleVolume` avant passage à zéro, le restaurer au
démute.

## F8 — P2 : durée affichable sous la forme `0:60`

**Fichiers :** `src/routes/+page.svelte:254` ; `src/lib/components/Comparison.svelte:31-32` ;
`src/lib/components/Spectrogram.svelte:104-108`.

**Constat (vérifié) :** `Math.round(s % 60)` peut produire 60 → `"0:60"` (identique dans les
trois fichiers).

**Changement demandé :** utiliser `Math.floor(s % 60)`, ou normaliser avant de construire la
chaîne (minutes += 1 si secondes = 60).

## F9 — P2 : `.opus` absent du sélecteur de fichiers

**Fichier :** `src/routes/+page.svelte:149` et `168` :
`extensions: ["flac", "mp3", "m4a", "aac", "alac", "wav", "ogg"]`.

**Changement demandé :** ajouter `"opus"` (en cohérence avec M5).

## F10 — P2 : bandeau TEMPORARY DIAGNOSTIC encore présent

**Fichier :** `src/routes/+page.svelte:45-60` (état `diag` + `refreshDiag`), `791-794`
(`<pre class="diag">` + `onerror`), `1551-1567` (style `.diag`). 3 occurrences du marqueur.

**Changement demandé :** supprimer tout ce qui est marqué TEMPORARY DIAGNOSTIC : état,
fonction, rendu, style. Si un instrument reste nécessaire, le placer derrière un mode de
développement explicite hors du build de production. Ne pas oublier que `onerror` du lecteur
(actuellement `refreshDiag`) doit être remplacé par le vrai `playbackError` (F3).

## F11 — P2 : messages backend affichés bruts (anglais) dans l'UI française

**Fichier :** `src/routes/+page.svelte:138,158` (`error = String(e)`), rendus l.406, 420.
`src/lib/i18n.svelte.ts:12-13` documente que les erreurs backend ne sont pas traduites.

**Changement demandé :** catégoriser les erreurs (au moins : fichier illisible, format non
supporté, décodage incomplet, permission) et les traduire via i18n, en conservant le message
brut en détail optionnel.

## F12 — P2 : scrubber : navigation clavier non verrouillée

**Fichier :** `src/routes/+page.svelte:759-773`.

**Constat (vérifié) :** `scrubbing` est armé uniquement sur `pointerdown` (l.759). Une
navigation clavier (flèches) déclenche `input` sans armer le verrou → `timeupdate` peut
écraser la position pendant l'appui.

**Changement demandé :** armer `scrubbing` sur `keydown` (et `pointerdown`), libérer sur
`keyup`, `pointerup`, `pointercancel` et `blur`. Tester pointeur, clavier, annulation OS et
sortie de fenêtre.

## F13 — P2 : comparaison sans preuves explicatives

**Fichier :** `src/lib/components/Comparison.svelte:241-247`.

**Constat (vérifié) :** verdict + score uniquement ; aucun indicateur (l'`evidence` de
`+page.svelte:459-463` n'existe pas dans le composant). Le lecteur reste lié au fichier
principal sans indiquer quel fichier est joué (`+page.svelte:30-32` documente ce choix).

**Changement demandé :** afficher les principaux indicateurs sous chaque verdict ; afficher
le nom du fichier actuellement joué (ou « lecture : A » / « lecture : B ») ; le lecteur peut
rester unique mais doit être clairement attribué.

## F14 — P2 : comparaison : meilleure valeur codée uniquement par la couleur

**Fichier :** `src/lib/components/Comparison.svelte:268-269` (`class:leads`), `450-452`
(`td.leads { color: var(--ok) }`).

**Changement demandé :** ajouter un texte/icône et un libellé ARIA (« marge supérieure pour
cette mesure ») en plus de la couleur.

## F15 — P2 : textes i18n trop catégoriques sur des heuristiques

**Fichier :** `src/lib/i18n.svelte.ts:289,307,312,314,332,334,342` (FR).

Exemples vérifiés :
- l.307 : « Le fichier **a été rembourré**, pas réellement remasterisé. »
- l.314 : « là où la **forme d'onde a été aplatie** plutôt que reproduite. »
- l.312 : « c'est la **fréquence d'échantillonnage annoncée qui est gonflée** »
- l.289 : « De l'**audio sans perte n'a aucun alignement** de ce genre. »

**Changement demandé :** remplacer par « compatible avec », « probablement », « indique un
padding possible », « peut indiquer », en affichant le taux observé / le périmètre de la mesure
quand il existe (en cohérence avec T1, D2, T9, T10).

## F16 — P2 : repères présentés comme des vérités universelles

**Fichier :** `src/lib/i18n.svelte.ts:332-343` et équivalents EN l.520-535 ;
`src/lib/components/Comparison.svelte:109-183`.

- `-14 LUFS` présenté comme la cible streaming (l.332) ;
- `> 0 dBTP` présenté comme certitude d'écrêtage ultérieur (l.334, affiché via
  `+page.svelte:559`) ;
- DR `>= 12` étiqueté « bonne » / sinon « fortement compressée » (l.342).

**Changement demandé :** qualifier ces valeurs de repères/conventions (« cible streaming
courante », « peut écrêter après conversion », « DR élevé/faible ») ; présenter la DR comme
mesure conventionnelle, pas comme note de qualité.

## F17 — P2 : spectrogramme : rôle clavier incohérent

**Fichier :** `src/lib/components/Spectrogram.svelte:147-156`.

**Constat (vérifié) :** `role="button"` + `tabindex="0"` inconditionnels (même sans `onSeek`
en comparaison) ; `handleSeekKeydown` (l.40) retourne tôt sans `onSeek` et ne gère ni Entrée
ni Espace. Pas de `role="slider"`/`aria-valuenow`/`min`/`max`.

**Changement demandé :**
1. Soit `role="slider"` avec `aria-valuenow`, `aria-valuemin`, `aria-valuemax` et flèches
   gauche/droite/Home/End quand `onSeek` existe ;
2. Soit ne rendre le conteneur focusable/contrôlable que si `onSeek` est présent ;
3. Si `role="button"` est conservé, gérer Entrée/Espace.

## F18 — P2 : accessibilité des mètres et bandes

**Fichiers :** `src/lib/components/Meter.svelte:61-70` ; `src/lib/components/BandLevels.svelte:50-52`.

**Constat (vérifié) :** `role="meter"` + `aria-valuenow/min/max` présents, mais 0 occurrence de
`aria-label`/`aria-labelledby`/`aria-valuetext` dans les deux fichiers ; BandLevels repose sur
`title` (l.52).

**Changement demandé :** passer un label au composant `Meter` (`aria-label`), ajouter
`aria-valuetext` (« -14 LUFS », « -1 dBTP »), fournir une liste visuellement masquée pour les
bandes.

## F19 — P2 : responsive incomplet

**Fichiers :** `src/routes/+page.svelte:369-395` (topbar, jusqu'à 5 boutons, sans `flex-wrap` ;
`minWidth: 560` dans `tauri.conf.json:18`), `1686-1705` (seul media query, ne touche ni la
topbar ni les tables), `601` (`<table class="channels">`) ; `Comparison.svelte:252`
(`<table class="delta">`).

**Changement demandé :** `flex-wrap` ou regroupement des actions secondaires ; conteneurs
`overflow-x: auto` autour des tables ; tester à 560 px et avec les textes FR les plus longs.

## F20 — P2 : contraste et compatibilité WebKit (`color-mix`)

**Fichiers :** `src/routes/+page.svelte:844,861` (`--ink-low` : rgba 0.38/0.42 — très faible),
`903` (`color-mix`) ; `Comparison.svelte:344,347,350` ; `Meter.svelte:115` ; `tauri.conf.json:45`
(`minimumSystemVersion: "10.15"`).

**Constat (vérifié) :** 5 usages de `color-mix(in srgb, …)` dans 3 fichiers alors que la cible
macOS 10.15 est annoncée (le support de `color-mix` dans WKWebView de 10.15 est incertain).

**Changement demandé :** fallbacks opaques avant chaque `color-mix` (règle `@supports` ou
déclaration de repli), couleurs texte opaques, vérification des contrastes WCAG AA en clair et
en sombre.

## F21 — P3 : `localStorage` du thème non protégé

**Fichier :** `src/routes/+page.svelte:73` (getItem) et `118` (setItem).

**Constat (vérifié) :** le thème n'a pas de try/catch, contrairement au volume (l.79-84, 236-240)
et à la langue (i18n.svelte.ts:37-42, 61-65) qui sont protégés.

**Changement demandé :** encadrer les deux accès.

## F22 — P3 : fallback absent pour un code d'indicateur inconnu

**Fichier :** `src/lib/i18n.svelte.ts:264-297` (FR) ; `src/routes/+page.svelte:461` (rendu).

**Constat (vérifié) :** le `switch (i.code)` FR n'a pas de `default` → `undefined` à l'exécution
pour un code inconnu (rendu vide/« undefined »). L'EN a un fallback naturel (`i.message`).
La protection compile-time (`api.ts:125-126`) ne couvre pas un backend plus récent.

**Changement demandé :** ajouter un `default` au switch FR qui retombe sur `i.message` (comme
l'EN), et éventuellement une validation runtime (voir IPC-F1).

---

# Partie 5 — Contrat IPC et pipeline

## IPC-F1 — P2 : types TypeScript sans validation runtime

**Fichiers :** `src/lib/api.ts:119-241` ; `src/lib/i18n.svelte.ts:264-297`.

**Constat (vérifié) :** `invoke()` ne valide pas les données reçues (hors plage, base64
invalide, enum inconnu, tailles de tableaux).

**Changement demandé :** validation légère à la frontière IPC (validateurs ciblés, pas
nécessairement une bibliothèque), vérification des plages/tailles/enums, fallback indicateur
inconnu (F22), golden JSON verrouillant le contrat. Voir `tauri-ipc-contract`.

## IPC-F2 — P2 : pas de version ni de qualité dans le contrat d'analyse

**Fichiers :** `src-tauri/src/analysis.rs:24-45` (`AnalysisResult`) ; `src/lib/api.ts:216-228`.

**Changement demandé :** ajouter des champs explicites (charger `tauri-ipc-contract` avant) :
- `analysis_version` (ex. "0.4.0" ou constante du pipeline) ;
- `decode_status` (complet / incomplet / échec partiel) — cf. T2 ;
- `measurement_quality` (agrégat de la confiance des mesures) ;
- `spectral_bandwidth_status` (mesuré / repli Nyquist / absent) — cf. T3 ;
- `confidence_kind` (`declared` pour `DeclaredLossy`, `inferred` sinon) — cf. T11.

## P1 — P2 : pic mémoire structurel du pipeline

**Fichiers :** `src-tauri/src/decode.rs:18-25` ; `src-tauri/src/analysis.rs:73-162` ;
`src-tauri/src/spectral.rs:206-212`.

**Constat (vérifié) :** le pipeline conserve tous les buffers de canaux + le buffer FFT
complet. Contexte du projet : ~845 Mo RSS pour 8 min en 96 kHz stéréo (`.claude/CONTEXT.md`).
Un fichier long en 192 kHz peut être tué par le système.

**Changement demandé (chantier long, à planifier, pas pour cette passe de correction) :**
décodage et réductions (RMS/peak/clipping/LUFS/DR) par blocs, spectrogramme downsamplé au fil
de l'eau, échantillon borné de fenêtres pour MDCT. (AGENTS.md acte que les analyses longues
tournent hors thread principal dès V0.1 — ne pas re-litiger ici.)

## P2 — P3 : pas de progression ni d'annulation

**Fichier :** `src-tauri/src/commands.rs:11-24` (commande `analyze_file`).

**Constat (vérifié) :** `spawn_blocking` évite de bloquer le thread événementiel mais aucune
progression ni annulation.

**Changement demandé (à planifier) :** `analysis_id`, events Tauri par étape (`app_handle.emit`),
token d'annulation, affichage de la phase courante.

---

# Partie 6 — Documentation, CI, outils

## Doc-1 — P2 : documentation obsolète

À mettre à jour (en anglais pour les docs publiques) :
- `README.md:53-57` : verdict à quatre états, pas trois (cf. T11).
- `README.md:113-120` : « AAC 256 totalement non détecté » faux — la grille MDCT le détecte
  sur le corpus.
- `src-tauri/tests/fixtures/corpus/README.md:88` : AAC 128 dynamique « indéterminé 30 % » faux —
  classé probablement transcodé 90 %.
- `src-tauri/src/transcode_detect.rs:6` : doc qui parle encore de trois états.
- `.claude/CLAUDE.md:31,174-178` : couverture AAC réelle (grille MDCT).
- `CHANGELOG.md:254-258` : « release en brouillon » vs `releaseDraft: false` dans le workflow.
- `src-tauri/src/spectral.rs:5-7` : commentaire disant que le scoring V0.3 n'existe pas alors
  qu'il existe.
- `src-tauri/Cargo.toml` : commentaire `protocol-asset` obsolète (feature retirée).
- `.claude/CONTEXT.md` : décrit encore `asset://` alors que le serveur loopback existe.
- `README.md:67-69` : lecture via serveur loopback, pas `asset://`.

## Doc-2 — P2 : workflow de release insuffisamment protégé

**Fichier :** `.github/workflows/release.yml:20-138`.

**Constat (vérifié) :** pas de dépendance explicite à la CI complète (build/test/clippy/check) ;
`releaseDraft: false` ; actions à tags flottants (`@v0`, `@stable`, `@v4`).

**Changement demandé (charger `release-packaging` avant) :**
1. Faire dépendre la release d'un job de validation identique à la CI PR (`build.yml`).
2. Publier en draft jusqu'à vérification manuelle des artefacts.
3. Pinner les actions critiques sur des SHA validés (ou au minimum documenter le risque).
4. Documenter le risque des builds non signés séparément de la qualité de l'analyse.

## Doc-3 — P3 : `cargo fmt --all -- --check` en échec

Corriger le formatage (au minimum les nouveaux fichiers : `media_server.rs` ; idéalement tout
le dépôt dans une PR dédiée `refactor/`). Ajouter `cargo fmt --check` à la CI (build.yml) après
réconciliation.

## Doc-4 — P3 : `tauri-plugin-opener` probablement inutilisé

Présent dans `Cargo.toml`, `package.json`, capabilities, initialisé dans `src-tauri/src/lib.rs`
(autour de l.20). Aucune utilisation trouvée. **Changement demandé :** le supprimer (dépendance
+ permission + initialisation) s'il n'existe aucun besoin, sinon ajouter la fonction qui le
justifie. Vérifier les capabilities Tauri associées.

## Doc-5 — P3 : exclusions Windows Defender très larges en CI

**Fichiers :** `.github/workflows/build.yml:52-61` ; `release.yml:74-85`. Accélère les runners
mais réduit la protection pendant le build. Documenter le compromis (commentaire déjà présent)
et le limiter aux runners de confiance.

## Doc-6 — P3 : outils de diagnostic non portables / codes de sortie trompeurs

- `.claude/tools/serve_main.rs:1` : `#[path = "/Users/matthieu/..."]` chemin absolu local →
  le rendre relatif/portable.
- `.claude/tools/wkplay.swift:31-33` : `exit(0)` même sur erreur → code de sortie non nul sur
  échec pour être utilisable en test automatisé.

---

# Ordre de correction recommandé

**Passe 1 — Bloqueurs (1 PR par préoccupation, branche + PR obligatoires, jamais de commit
direct sur `main`) :**

1. M1 (Windows), M2 (range underflow), M3 (DoS bornes) — `fix/` sur `media_server.rs`.
2. T1 (verdict no-edge → Indeterminate) — `fix(transcode)` + **relance du corpus obligatoire**
   avec rapport FP/FN avant/après.
3. T2 + IPC-F2 `decode_status` (décodage incomplet → verdict neutralisé) — `feat(ipc)!:` (le
   contrat change).
4. F1-F3 (races, Promise.all, play() catch) + suppression du diagnostic temporaire (F10) —
   `fix(ui)`.

**Passe 2 — Exactitude des mesures :** T3-T10, D1-D9, avec tests numériques et corpus.

**Passe 3 — Frontend/accessibilité :** F4-F9, F11-F22.

**Passe 4 — IPC, docs, CI :** IPC-F1, Doc-1 à Doc-6, fmt.

# Commandes de vérification (à la fin de chaque PR)

```bash
# dans src-tauri/
cargo build --locked
cargo test --locked --release -- --nocapture     # inclut le rapport corpus FP/FN
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check

# à la racine
npm run check
npm run build
npm run tauri build -- --debug    # bout en bout macOS
```

# Définition de fini (critères d'acceptation)

- Le serveur démarre sur macOS et Windows ; aucun chemin Unix en code non conditionné.
- Aucune requête malformée ne provoque de panic, d'allocation non bornée ou de réponse
  incohérente (ranges inversés/suffixes/malformés → 416 ou réponse documentée).
- Aucune URL média ne permet de sélectionner un chemin non autorisé (pas de TOCTOU symlink).
- Tout fichier supporté par l'analyse a un MIME de lecture validé (dont Opus).
- Corpus : 0 faux positif ; les deux LAME V0 ne sont plus `ProbablyAuthentic` (rapport
  avant/après fourni).
- Un décodage incomplet ne produit plus jamais de verdict affirmatif (indicateur + état
  `decode_status`).
- `spectral_cutoff_hz` ne peut plus être confondu avec une mesure (repli Nyquist distingué).
- Deux analyses concurrentes ne mélangent pas leurs résultats ; une erreur audio est visible
  comme erreur de lecture sans effacer un rapport valide.
- Aucun diagnostic temporaire, chemin absolu de développeur ou banc cassé ne reste dans la
  livraison.
- `cargo build/test/clippy/fmt`, `npm run check/build`, `npm run tauri build -- --debug`
  passent ; le workflow Windows CI passe.
- README, CHANGELOG, commentaires et `.claude/CONTEXT.md` cohérents avec l'état réel.
- Une lecture réelle d'un FLAC > 32 MiB va au bout au premier essai (fenêtre active et
  masquée), avec logs WebKit conservés si le problème réapparaît — sans jamais présenter la
  reprise comme preuve que la cause WebKit est corrigée.
