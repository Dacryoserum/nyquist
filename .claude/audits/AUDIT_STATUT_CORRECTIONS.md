# Nyquist — état des corrections d'audit (livré en v0.5.0)

Date : 2026-08-28. Livré dans la version **0.5.0**. Référence : `AUDIT_CORRECTION_EXECUTABLE.md` (V3), qui fait foi.
`AUDIT_V2_PLAN_CORRECTION.md` conservé pour l'historique, `AUDIT_COMPLET.md` marqué périmé.

Travail non committé sur `main`, comme le worktree qu'il corrige.

## Vérifications (toutes vertes)

| Commande | Résultat |
|---|---|
| `cargo build --locked` | ✅ |
| `cargo test --locked --release` | ✅ 35 unit + 2 calibration + 7 corpus |
| `cargo clippy --locked --all-targets -- -D warnings` | ✅ |
| `cargo fmt --all -- --check` | ✅ (échouait sur ~10 fichiers avant) |
| `npm run check` | ✅ 0 erreur, 0 warning |
| `npm run build` | ✅ |

## Corpus de transcodage — avant / après

20 fixtures. Le chiffre qui a changé est le dernier.

| | avant | après |
|---|---|---|
| Faux positifs (authentique accusé) | 0 | **0** |
| Faux négatifs inattendus | 0 | **0** |
| Ratés documentés (LAME V0) | 2 | **2** |
| **Fichiers réellement transcodés déclarés « probablement authentique »** | **2** | **0** |

Les deux LAME V0 (`transcoded_mp3_v0_44k`, `transcoded_dynamic_mp3_v0_44k`) passaient
`ProbablyAuthentic` à 0,65 — l'outil cautionnait activement un faux. Ils sortent maintenant
`Indeterminate` à 0,30. Ils restent des ratés : la méthode ne les voit toujours pas, mais elle
ne se prononce plus.

Contrepartie assumée : tout fichier 44,1/48 kHz sans coupure détectable sort désormais
`Indeterminate`, y compris les authentiques. C'est la réponse honnête — à ces fréquences un
master lossless et un transcodage LAME V0 sont indiscernables par toute méthode présente dans
ce projet. `ProbablyAuthentic` demande maintenant une preuve positive mesurée
(`spectral::above_cd_ceiling_db`), que seuls les fichiers ≥ 88,2 kHz peuvent fournir.

Le seuil de cette preuve est calibré sur le corpus, avec une séparation structurelle et non
fortuite : le filtre anti-repliement d'un ré-échantillonneur laisse le haut de la nouvelle
bande vide par construction.

| fixture | haut de bande vs référence 1–22 kHz |
|---|---|
| `authentic_96k_noise` (vrai hi-res) | −0,03 dB |
| `authentic_musiclike_96k` (vrai hi-res) | −18,6 dB |
| `upsampled_44k_to_96k` (sur-échantillonné) | −47,7 dB |
| `transcoded_mp3_128_upsampled_96k` | −64,2 dB |

Seuil à −30 dB : 11 dB de marge sous le cas authentique le plus faible, 18 dB au-dessus du
faux le plus fort. La première version de cette mesure lisait toute la bande au-dessus de
22,05 kHz et ne séparait que de 4 dB — insuffisant pour autoriser un verdict.

## Corrigé

### Lecteur audio — remplacé, pas rafistolé

Trois symptômes signalés (seek qui tombe à côté, compteur incohérent, arrêt avant la fin)
avaient **une seule cause** : l'élément `<audio>` du webview se forgeait sa propre durée en
parsant le fichier, pendant que le curseur (`+page.svelte:990`), l'axe du spectrogramme et
le rapport utilisaient celle du décodeur. Le seek écrivait une fraction de *notre* durée
dans `currentTime`, que l'élément interprétait contre *la sienne*.

`media_server.rs` corrigeait le transport, pas la divergence. Deux horloges ne se
synchronisent pas en améliorant le coursier entre elles.

**`src-tauri/src/player.rs`** joue maintenant les échantillons que l'analyse vient de
décoder, via `rodio`/`cpal`. Une seule horloge : un index d'échantillon. Conséquences :

- seek exact à l'échantillon près ; position dérivée de ce qui est réellement envoyé au
  périphérique, donc elle ne peut pas dériver ;
- `media_server.rs` **supprimé** (679 lignes) — et avec lui M1 à M6, le port ouvert, le
  jeton, les plages d'octets, la CSP passe à `media-src 'none'` ;
- la reprise automatique supprimée : elle contournait un défaut qui ne peut plus survenir ;
- les bancs `wkplay.swift` et `serve_media.rs` supprimés, faute de sujet.

Coût assumé : la piste décodée reste en mémoire tant qu'elle est chargée (~10 Mo/minute
stéréo en 44,1 kHz), là où elle était libérée après l'analyse.

Licences vérifiées avant ajout, comme l'exige `AGENTS.md` : rodio et coreaudio-rs en
MIT/Apache-2.0, cpal en Apache-2.0. Aucune décision de licence à rouvrir. **ffmpeg a été
écarté** : c'est une bibliothèque de décodage, pas de sortie audio (il faudrait quand même
une couche device par-dessus), elle remplacerait `symphonia` qui fait déjà le travail, et
elle est LGPL — ce qui aurait justement rouvert la question réservée par `AGENTS.md`.

### Serveur média (M1–M6) — corrigé puis supprimé

Les corrections ci-dessous ont été faites et testées avant que le module ne soit retiré au
profit de la lecture native. Elles sont conservées ici pour la traçabilité de l'audit ; le
code correspondant n'existe plus.


- **M1** — `getrandom` remplace `/dev/urandom` codé en dur ; `MediaServerState` porte l'erreur
  de démarrage au lieu de paniquer. L'app démarre sous Windows et fonctionne (analyse seule) si
  le bind échoue.
- **M2** — `parse_range` a trois états (`Absent` / `Valid` / `Suffix` / `Invalid`). `bytes=100-50`
  → `416` au lieu d'un underflow `u64`. Les ranges suffixes sont implémentées. Multi-range est
  ignoré (RFC 9110 §14.2), décision documentée et testée.
- **M3** — plafond de connexions (32, `503` au-delà), request-line et headers bornés (8 KiB,
  `431`), nombre de headers borné, write timeout, `lingering_close` sur refus.
- **M4** — **l'URL ne contient plus de chemin.** L'autorisation ouvre le fichier ; l'URL désigne
  ce descripteur par un identifiant aléatoire. Lectures par offset (`read_at`/`seek_read`), donc
  pas de curseur partagé entre requêtes concurrentes. Plus de fenêtre TOCTOU : il n'y a plus de
  nom à ré-résoudre.
- **M5** — MIME déterminé par les premiers octets du fichier réellement ouvert, avec repli sur
  l'extension. `.opus`/`.oga` ajoutés au sélecteur. Un `.flac` renommé `.bin` est servi
  `audio/flac` au lieu d'`application/octet-stream`. Le commentaire faux est supprimé.
- **M6** — URL neuve par autorisation → ressource immuable, `max-age` long légitime,
  `If-None-Match` → `304`. Longueur et mtime épinglées et revérifiées : un fichier réécrit en
  place fait répondre `410 Gone` plutôt que de contredire un cache.

**Tests ajoutés** (17 au total dans le module) : bornes de range (`0-0`, `len-1`, `start>=len`,
ouverte, suffixe, suffixe trop long, inversée, multi-range, caractères invalides, fichier vide,
`HEAD` avec et sans range), livraison complète d'un fichier de **40 Mio** (le défaut d'origine,
à son échelle), déconnexion client en cours de réponse, requêtes surdimensionnées, plafond de
connexions et récupération, plafond de ressources vivantes, remplacement de fichier, unicité des
identifiants.

### Verdict et DSP

- **T1** — absence d'indice → `Indeterminate`. Voir le tableau ci-dessus.
- **T2** — `decode::DecodeStatus` au contrat ; `ResetRequired` marque désormais le décodage
  incomplet (il était silencieux) ; un décodage incomplet force `Indeterminate` avec un
  indicateur explicite, les mesures restant affichées.
- **T3** — `spectral_cutoff_hz` devient `Option<f64>`. Le repli sur Nyquist est supprimé :
  `authentic_bass_only_44k` ne prétend plus occuper 22,05 kHz à 100 %. `sample_rate.rs` ne
  conclut plus sur une mesure inexistante.
- **T4** — `find_spectral_edge` garde les 5 meilleurs candidats (dédupliqués par la largeur de
  sonde) et applique les portes à chacun, au lieu de tout jouer sur le plus profond.
- **T6** — la confiance s'affiche en force d'indices (faible/modérée/forte), UI et CLI. Le
  nombre brut reste dans le JSON.
- **T10** — les tags sont dédupliqués par `(clé, valeur)` : un tag contenant deux motifs comptait
  pour deux. La non-couverture des tags de fin de fichier (ID3v1/APEv2) est dite dans le message.
- **T11** — `confidence_score` devient `Option`, `None` pour `DeclaredLossy` ; CLI et UI alignés.
- **D2** — seuil d'écrêtage dérivé de la profondeur déclarée (en 24 bits l'ancien seuil fixe
  comptait les 256 valeurs les plus hautes comme écrêtées), et `full_scale_sample_count` séparé
  de `clipped_run_count` (suites ≥ 3).
- **D3** — l'erreur `ebur128` est propagée au lieu de devenir `-120 dBTP` ;
  `true_peak_oversampling` exposé et affiché — à 192 kHz c'est un pic échantillonné, pas une
  crête inter-échantillon.
- **D4** — l'alignement de grille ne porte plus que sur les échantillons actifs ;
  `active_sample_ratio` exposé ; résultat refusé sous 1 % d'activité.
- **D6** — LRA `None` sous 10 s (la bibliothèque renvoyait un 0.0 fini qui passait pour une
  mesure).
- **D7** — `effectively_mono` était **inatteignable** : le test comparait la valeur écrêtée au
  plancher d'écrêtage. Valeur brute et valeur d'affichage séparées.

### Frontend

F1 (jeton de génération sur analyse, lecteur et reprise), F2 (`Promise.allSettled` — le rapport
s'affiche même si la lecture échoue), F3 (`play()` capturé, `playbackError`, `error`/`stalled`/
`canplay`), F4 (`teardownPlayback` : pause + détachement avant remplacement), F5 (`compareError`
distinct, rendu unique de l'erreur principale), F6 (export try/catch + confirmation), F7 (volume
mémorisé au mute), F8 (`0:60` — trois fichiers), F9 (`.opus`), **F10 (bandeau de diagnostic
temporaire supprimé)**, F11 (erreurs backend catégorisées et traduites), F12 (scrubber verrouillé
au clavier + capture de pointeur), F13 (indices sous chaque verdict de comparaison + fichier en
lecture nommé), F14 (marqueur + libellé ARIA, plus seulement la couleur), F15/F16 (formulations
nuancées, repères présentés comme conventions), F17 (`role="slider"` avec `aria-valuenow`,
focusable seulement si `onSeek`), F18 (`aria-label`/`aria-valuetext` sur `Meter`, liste masquée
pour les bandes), F19 (topbar `flex-wrap`, tables en `overflow-x`), F20 (repli opaque avant chaque
`color-mix`), F21 (`localStorage` du thème protégé), F22 (`default` sur le switch FR).

### Contrat, docs, CI, outils

- **IPC-F2** — `analysis_version` et `decode_status` ajoutés au contrat.
- **Doc-1** — README (4 états, couverture AAC réelle, serveur loopback), `CHANGELOG.md` (cause
  WebKit présentée comme hypothèse), `transcode_detect.rs`, `spectral.rs`, `Cargo.toml`,
  `.claude/CLAUDE.md`, `.claude/CONTEXT.md`, `corpus/README.md` (dont une ligne AAC 128 déjà
  fausse avant cette passe), `AUDIT_COMPLET.md` marqué périmé.
- **Doc-2** — release en **draft**, et `build.yml` devient `workflow_call` réutilisé par
  `release.yml` : plus de release construite depuis un commit que la CI rejetterait.
- **Doc-3** — `cargo fmt --all` appliqué, et un job `format` ajouté à la CI.
- **Doc-4** — `tauri-plugin-opener` supprimé (dépendance, permission, initialisation,
  `package.json`) — aucun code ne s'en servait.
- **Doc-6** — `serve_main.rs` (chemin absolu vers un dossier personnel) devient
  `src-tauri/examples/serve_media.rs`, un exemple cargo qui hérite des dépendances du crate ;
  `wkplay.swift` sort en code non nul sur erreur **et** sur playhead bloqué (il disait `END`).

## Détection MP3 transparent (LAME V0) — tentative, résultat négatif, et le vrai blocage

Objectif : pouvoir déclarer authentique un vrai fichier 44,1 kHz, ce qui exige un indicateur
positif capable de voir un V0 — sans quoi relâcher le verdict revient à cautionner les faux.

**Piste testée** : nulls alignés sur le réseau MP3 (32 sous-bandes de `rate/64`, granules de
576 échantillons), balayés sur les 576 offsets, méthodologie identique à `mdct_grid`.
Implémentée, mesurée, **retirée**.

Niveau par sous-bande relatif à la moyenne du granule, en dB :

| fixture | min | p1 | p10 |
|---|---|---|---|
| `transcoded_mp3_v0_44k` (**faux**) | −8,5 | −5,4 | −2,9 |
| `authentic_44k_noise` (vrai) | −8,4 | −5,4 | −2,9 |
| `transcoded_dynamic_mp3_v0_44k` (**faux**) | −31,6 | −23,9 | −18,4 |
| `authentic_dynamic_stereo_44k` (vrai) | −28,7 | −23,6 | −18,8 |

LAME V0 ne vide aucune sous-bande : ses statistiques sont celles du lossless, chiffre pour
chiffre. Les nulls profonds observés ailleurs (`mp3_128` à −94 dB, `mp3_320` à −73) suivent
exactement le passe-bas, déjà détecté. Et `authentic_44k_tonal`, authentique, atteint −63,9 dB
à son 25ᵉ centile : un comptage de trous l'accuserait — le piège déjà consigné dans
`corpus/README.md` pour la tentative « trous spectraux », reproduit à l'identique.

**Le vrai blocage n'est pas l'algorithme, c'est le corpus.** Toutes les fixtures viennent
d'`anoisesrc`. Le bruit est la matière sur laquelle un encodeur perceptuel jette le moins :
incompressible, non masquable, donc à V0 il dépense des bits partout et ne laisse rien.
Aucun détecteur V0, correct ou faux, ne peut être validé ni réfuté ici.

**Livré à la place** : `src-tauri/tests/local_probe.rs`, un banc de mesure qui rapporte, pour
tout fichier déposé dans `corpus/local/` (gitignoré), les statistiques qu'un détecteur devrait
séparer. Pointé sur un original lossless et son propre transcodage V0, il répond directement à
la question. Passe trivialement quand le dossier est vide.

**Chemin restant si la mesure est favorable** : Derrien 2019 cherche l'**erreur de
quantification** — des coefficients décodés qui se regroupent sur les niveaux de
reconstruction du quantificateur — présente dans *tout* coefficient que l'encodeur a touché, y
compris ceux qu'il a gardés. Pour le MP3 il faut inverser réellement le banc hybride : étage
polyphase 32 bandes (fenêtre prototype normative de 512 coefficients, ISO/IEC 11172-3
table B.3 — à sourcer, pas à écrire de mémoire ; celle de symphonia est sous MPL-2.0, ce qui
poserait une question de licence), puis MDCT-18 par sous-bande avec les quatre types de
fenêtre, réduction d'alias inverse, et estimation du pas de quantification par bande. C'est un
chantier de recherche, pas un seuil.

## Non corrigé, et pourquoi

- **D1 (LUFS multicanal sans layout)** — demande de mapper les positions Symphonia vers
  `ebur128::Channel` et de valider contre libebur128/ffmpeg sur du 5.1/7.1. Aucune fixture
  multicanal dans le corpus : livrer un mapping non validé sur du code BS.1770 serait exactement
  ce que la skill `dsp-correctness` interdit. Le projet n'analyse en pratique que du stéréo.
- **D5 (plancher −120 dB → `null`)** — churn de contrat large pour un gain d'honnêteté modeste ;
  à faire dans une passe dédiée avec D8.
- **D8 (longueurs de canaux incohérentes)**, **D9 (hypothèses spectrales)** — chantiers de mesure,
  pas des corrections ponctuelles.
- **T5 (scoring temporel)** — option A est un vrai changement de scoring qui demande sa propre
  validation corpus ; à ne pas mêler à T1, dont l'effet doit rester lisible.
- **T7 (périmètre MDCT)** — les textes sont nuancés (« AAC à blocs longs, un seul canal »), mais
  le choix du canal le plus énergétique et l'état `unsupported_profile` restent à faire.
- **T8 (plancher 8 kHz)**, **T9 (fixtures hi-res naturellement sombres)**, **T12 (liste de codecs)**
  — demandent de nouvelles fixtures avant tout changement de seuil.
- **IPC-F1 (validation runtime du JSON)** — F22 couvre le cas concret (indicateur inconnu) ; la
  validation générale reste à faire.
- **P1 (pic mémoire)**, **P2 (progression/annulation)** — l'audit les qualifie lui-même de
  chantiers à planifier.

## Ce qui n'est pas prouvé

La cause racine côté WebKit de la troncature n'est **pas** démontrée. Ce qui est mesuré : ce
transport livre un fichier de 40 Mio en entier là où les deux autres s'arrêtaient vers 32 Mio
(test automatisé, `a_file_past_the_32_mib_ceiling_is_delivered_whole`). Le mécanisme interne
reste une hypothèse. La reprise automatique est un contournement, et l'app dit maintenant quand
elle a dû intervenir sans y parvenir, plutôt que de laisser une lecture reprise passer pour
normale.

La validation manuelle restante, non faite ici : une lecture réelle d'un FLAC > 32 Mio dans
l'app, fenêtre active puis masquée, en vérifiant que le compteur de reprises reste à zéro. Et
le passage CI Windows, qui ne peut être vérifié que sur un runner Windows.
