@../AGENTS.md

# Nyquist — Analyseur de qualité audio

Application desktop (Tauri/Rust + Svelte) qui analyse un fichier audio et détecte s'il a
été transcodé depuis une source lossy en dépit d'une étiquette lossless (FLAC/ALAC/WAV
gonflé depuis un MP3/AAC). Sert aussi d'outil général d'inspection (métadonnées, intégrité,
caractéristiques du signal).

> **Règles de travail** : importées ci-dessus depuis `AGENTS.md` (racine).
> **Pièges durables** : `.claude/CONTEXT.md`. **Skills projet** : `.claude/skills/`.
> Ce fichier décrit *ce qu'est le projet* ; `AGENTS.md` décrit *comment y travailler*.
> Le projet est open source, doc publique et commits en anglais — voir AGENTS.md § Barre
> de qualité. Ce fichier-ci et les skills restent en français : ils s'adressent à toi et
> aux agents de code, pas aux contributeurs externes.

---

## État du projet

**Phase actuelle : V0.1 → V0.3 (premiers slices) + lecture audio + un lot de
fonctionnalités "audiophile avancé", tous en place.** Décodage (symphonia), métadonnées,
RMS/peak/DR14/LUFS/LRA/true peak, vérification d'intégrité FLAC (MD5 embarqué), détection
de bit-depth padding ("faux hi-res"), empreinte d'encodeur lossy dans les tags,
spectrogramme FFT avec spectral cutoff + pente de rolloff + coupure dans le temps, scoring
de transcodage 4 états, lecture audio native avec clic-pour-naviguer, export JSON, et un
binaire CLI (`nyquist-cli`) pour l'usage scripté/batch. UI dashboard (thème sombre chaud,
cartes, icônes, canvas spectrogramme + dégradé inferno) — validée visuellement et
fonctionnellement en conditions réelles par l'utilisateur.

**Le verdict repose sur deux recherches structurelles, une par famille de codec.**
`mdct_grid.rs` ré-analyse le signal sur la MDCT de l'AAC (1024 décalages, fenêtres sinus et
KBD) ; `mp3_grid.rs` le repasse dans le banc de filtres hybride du MP3 (polyphase 32 bandes
avec la fenêtre normative ISO 11172-3, MDCT 36 points par sous-bande, inversion de fréquence,
papillons anti-repliement) aux 576 alignements de granule. Même statistique des deux côtés :
la part de coefficients à plus de 40 dB sous le RMS de leur propre voisinage — l'empreinte
d'un quantificateur, visible au seul alignement de l'encodeur. Seuil 12 sur le balayage
**et** sur des trames de confirmation distinctes.

- Grille confirmée → `ProbablyTranscoded`. **LAME V0 n'est plus un point aveugle** : le test
  lit la quantification, pas l'enveloppe spectrale.
- Deux recherches propres (< 6), fréquence ≤ 48 kHz, pas de coupure franche inexpliquée, pas
  de tag d'encodeur lossy, décodage complet → `ProbablyAuthentic` (force plafonnée à 0.7).
  C'est un **négatif testé**, pas une absence d'indice : les mêmes recherches attrapent les
  fichiers que l'ancienne règle cautionnait.
- Sinon `Indeterminate`, avec la raison précise dans `indicators[]`.

Un spectre seul n'accuse ni ne blanchit jamais ; les tags ne peuvent que suspendre. La
confiance n'est **pas** une probabilité : l'UI et le CLI l'affichent en force d'indices
(faible/modérée/forte), le nombre brut ne survit que dans le JSON.

Validation sur de la vraie musique (14 enregistrements commerciaux indépendants, 98 dérivés
LAME V0/V2/128/192/320 et AAC 256/192) : **98/98 détectés, 0/13 sources accusées** — voir
`docs/detection-research.md`. Chiffres de développement, pas un taux tenu hors échantillon.
Corpus versionné (`tests/corpus_smoke.rs`) : **20 fixtures, 8/10 dérivés détectés, 2 échecs
documentés laissés indéterminés, 0 faux positif, 0 transcodé cautionné**.

⚠️ **Ce que `ProbablyAuthentic` ne couvre pas** : un fichier lossy ré-échantillonné ou
retraité après décodage (la grille disparaît), un codec hors MP3/AAC (Opus, Vorbis — d'où la
suspension sur coupure franche inexpliquée), et tout fichier au-delà de 48 kHz (jamais
blanchi). Bruit blanc à 320 kbps : aucun coefficient supprimé, donc rien à trouver. Pistes
rejetées avant d'en arriver là (trous spectraux, grille temporelle, stéréo joint, grille de
sous-bandes en FFT) consignées dans `tests/fixtures/corpus/README.md`.

Un décodage incomplet (paquets sautés, `ResetRequired`, FLAC tronqué par rapport à son
en-tête) suspend le verdict : les mesures restent, le jugement est retiré
(`decode::DecodeStatus`).

**Le bit-depth padding ("faux hi-res") est un problème séparé, avec sa propre méthode** —
alignement exact sur une grille de quantification plus grossière que la profondeur déclarée
(pas une estimation de bruit de plancher/SNR, voir `bit_depth.rs`). Validé sur 2 fixtures
dédiées (16-bit zero-paddé en 24-bit vs. vraiment 24-bit) : 0 faux positif sur tout le
corpus existant.

`cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `npm run check`
et `npm run build` passent tous. Perf mesurée : ~2.4s en release pour le pipeline complet
sur un FLAC 6:52/24-bit réel (voir CONTEXT.md — DR14 et bit-depth ont un coût réel, encore
sous le seuil qui justifierait de la progression). Pas encore fait : marqueurs automatiques
sur le lecteur (clipping, etc. — écarté du scope actuel à la demande explicite de
l'utilisateur), détection MQA (explicitement écartée — profil de risque différent, sujet
contesté, pas assez de recherche validée pour l'implémenter correctement). Pas de release
publique, pas d'utilisateurs, pas de dette à préserver : les décisions techniques peuvent
encore bouger, mais celles listées dans `AGENTS.md` sous « Décisions actées » sont
tranchées et ne doivent pas être re-débattues sans raison nouvelle.

---

## Architecture

SvelteKit en mode SPA statique (`adapter-static`, `ssr = false` — Tauri n'a pas de serveur
Node, voir `src/routes/+layout.ts`), pas un simple Svelte+Vite.

```
nyquist/
├── src-tauri/                  # Backend Rust
│   ├── src/
│   │   ├── main.rs             # Point d'entrée Tauri (GUI)
│   │   ├── lib.rs              # Builder Tauri, plugins, enregistrement des commandes
│   │   ├── bin/nyquist-cli.rs  # ✅ Binaire CLI headless (scripting/batch), même pipeline
│   │   ├── analysis.rs         # ✅ Pipeline complet partagé entre commands.rs et le CLI
│   │   ├── decode.rs           # ✅ Décodage symphonia + intégrité MD5 + tags encodeur
│   │   ├── metadata.rs         # ✅ Métadonnées techniques (pas les tags de titre/artiste)
│   │   ├── tags.rs             # ✅ Empreinte d'encodeur lossy dans les tags conteneur
│   │   ├── signal_analysis.rs  # ✅ RMS, peak, true peak, LUFS, LRA, crest factor, clipping
│   │   ├── dynamic_range.rs    # ✅ DR14 (Pleasurize Music Foundation), voir module docs
│   │   ├── bit_depth.rs        # ✅ Détection bit-depth padding ("faux hi-res")
│   │   ├── spectral.rs         # ✅ FFT, spectrogramme downsamplé, cutoff + pente + dans le temps
│   │   ├── mdct_grid.rs        # ✅ Grille de quantification MDCT d'un encodeur AAC (structurel)
│   │   ├── mp3_grid.rs         # ✅ Grille de quantification MP3 via son banc de filtres hybride
│   │   ├── mp3_window.rs       # ✅ Fenêtre polyphase normative (ISO/IEC 11172-3 Table B.3)
│   │   ├── stereo.rs           # ✅ Corrélation L/R, side/mid global et par bande, dual-mono
│   │   ├── transcode_detect.rs # ✅ Scoring 4 états, voir la skill dédiée + module docs
│   │   ├── player.rs           # ✅ Lecture native (rodio) depuis les échantillons décodés
│   │   └── commands.rs         # ✅ analyze_file, authorize_playback, export_report
│   ├── tests/
│   │   ├── calibration.rs      # ✅ Sinus à valeur RMS/peak/DR14 calculable à la main
│   │   ├── corpus_smoke.rs     # ✅ Corpus + rapport FP/FN transcodage + bit-depth
│   │   └── fixtures/
│   │       ├── generate_corpus.sh   # Régénère le corpus depuis zéro (ffmpeg)
│   │       └── corpus/              # ✅ voir corpus/README.md pour la vérité-terrain
│   ├── capabilities/default.json
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                         # Frontend SvelteKit (SPA)
│   ├── routes/+page.svelte      # ✅ Sélection, dashboard, lecteur audio, verdict, export
│   ├── routes/+layout.ts        # ssr = false (obligatoire pour Tauri)
│   ├── lib/api.ts               # ✅ Wrapper typé des appels IPC (miroir du contrat Rust)
│   ├── lib/colormap.ts          # ✅ Dégradé inferno (quiet→loud), partagé canvas + légende
│   ├── lib/icons.ts              # ✅ Set d'icônes SVG minimalistes (pas de dépendance externe)
│   └── lib/components/
│       ├── Icon.svelte          # ✅ Wrapper SVG générique pour icons.ts
│       └── Spectrogram.svelte   # ✅ Canvas + axes + légende + clic-pour-naviguer + playhead + trace de coupure
├── .github/
│   ├── workflows/build.yml      # ⏳ pas encore créé — CI à ajouter avant le premier vrai PR
│   └── PULL_REQUEST_TEMPLATE.md
├── AGENTS.md
├── CONTRIBUTING.md
├── CHANGELOG.md
└── LICENSE                      # MIT
```

✅ = existe et compile (`cargo build`/`cargo clippy -- -D warnings`/`npm run check` verts).
⏳ = prévu, pas encore écrit — ne pas créer de stub vide, l'écrire quand sa phase arrive.

## Stack (résolue — voir AGENTS.md § Décisions actées pour le raisonnement)

| Besoin | Crate | Note |
|---|---|---|
| Décodage universel | `symphonia` 0.6 | FLAC/MP3/AAC/ALAC(isomp4)/WAV/OGG confirmés via `features = ["all"]`. **API 0.6 différente des exemples 0.5.x qu'on trouve en ligne** — voir `.claude/CONTEXT.md` avant de retoucher `decode.rs`. |
| FFT / spectrogramme | `rustfft` | STFT Hann-windowée, downsamplée en Rust avant envoi (voir `spectral.rs`). |
| Encodage compact IPC | `base64` | Spectrogramme transmis en u8 quantifié + base64, jamais en matrice JSON dense. |
| LUFS + True Peak | `ebur128` 0.1 | Port Rust pur de libebur128, MIT. API confirmée dans `.claude/CONTEXT.md`. |
| Lecture audio | `rodio` 0.22 (sur `cpal`) | Lecture **native**, alimentée par les échantillons que l'analyse vient de décoder — le webview ne reçoit plus aucun média. Deux tentatives via l'élément `<audio>` (protocole `asset://` puis serveur HTTP loopback) ont échoué : l'élément se forge sa propre durée, qui contredit celle du décodeur, d'où seek faux, compteur qui dérive et arrêts prématurés. Une seule horloge désormais : un index d'échantillon. MIT/Apache-2.0. Voir `player.rs` et `.claude/CONTEXT.md` avant d'y toucher. |
| Sélecteur de fichier | `tauri-plugin-dialog` | Utilisé par `+page.svelte`. |
| Historique local (V2.0+) | `rusqlite` | Pas encore ajouté — schéma à figer avant l'implémentation. |
| Décodage FLAC bas niveau | `claxon` | Différé — seulement si une fonctionnalité de vérification d'intégrité bit-exact FLAC est actée. |

## Contrat de données (backend → frontend)

Défini par `AnalysisResult` dans `src-tauri/src/analysis.rs` (miroir TypeScript dans
`src/lib/api.ts`), partagé entre la commande Tauri `analyze_file` et le CLI :
`file_info` (+ `integrity_verified`) + `signal_analysis` (+ `loudness_range_lu`) +
`dynamic_range` (DR14) + `spectral_analysis` (cutoff, pente de rolloff, cutoff dans le
temps, `spectrogram` : intensité u8 quantifiée downsamplée — `TARGET_TIME_BINS`/
`TARGET_FREQUENCY_BINS` dans `spectral.rs` — puis base64-encodée ; jamais la matrice dense,
voir la skill `tauri-ipc-contract`) + `transcode_assessment` (verdict 4 états +
`confidence_score` + `indicators[]` : chaque indice porte la prose anglaise du backend
(`message`, ce que voient le CLI et le JSON exporté) **et** son `code` + ses mesures brutes,
pour que l'UI recompose la phrase en français — ajouter une variante à `IndicatorDetail`
oblige à traduire dans `i18n.svelte.ts`, sinon `npm run check` échoue) +
`encoder_tag_matches` + `mdct_grid` + `mp3_grid` (profil de balayage en base64, scores,
décalage) + `bit_depth_analysis` + `stereo_analysis` (corrélation L/R, side/mid
par bande, dual-mono exact — `null` si le fichier n'est pas exactement stéréo).
Payload mesuré ~240KB pour un FLAC de 6:52, calcul total ~2.4s en release (voir CONTEXT.md).
Deux autres commandes IPC : `authorize_playback(path)`, qui autorise le fichier **et renvoie
l'URL** qui le joue (une seule opération : aucune URL n'existe pour un fichier non autorisé),
et `export_report(path, json)` (le frontend sérialise et appelle `save()` lui-même, le backend
ne fait qu'écrire le fichier).

## Roadmap

1. **V0.1 — Cœur d'analyse minimal** ✅ : décodage (symphonia), métadonnées, RMS/peak/DR,
   UI liste brute. Async (`spawn_blocking`) dès le départ. Progression via events Tauri pas
   encore câblée — pas nécessaire à ce stade : mesuré ~1.3s en release sur un FLAC de 6:52
   24-bit (voir `.claude/CONTEXT.md`), largement sous le seuil qui la justifierait. À
   reconsidérer si le spectrogramme (V0.2) s'avère significativement plus lourd.
2. **V0.1.5 — Corpus de test** ✅ : corpus synthétique reproductible (`generate_corpus.sh`)
   avec vérité-terrain connue, pas de fichiers commerciaux (voir corpus/README.md).
3. **V0.2 — Spectrogramme** ✅ (premier slice) : FFT, génération des données, rendu canvas,
   spectral cutoff + pente de rolloff. Reste possible pour la suite : per-channel
   spectrogram (V0.2 actuel fait un downmix mono), échelle de fréquence log plutôt que
   linéaire.
4. **V0.3 — Détection de transcodage** ✅ (premier slice) : scoring 4 états basé sur la
   pente de rolloff + empreinte de tags encodeur, indices explicites, validé contre le
   corpus (0 FP, 2 échecs documentés). Point aveugle restant : LAME V0/AAC256, aucun
   lowpass à détecter par cette méthode — reste ouvert (pas de nouvel indicateur qui le
   couvrirait spécifiquement identifié pour l'instant).
5. **Lecture audio** ✅ (anticipée depuis V0.4) : lecture native `rodio` depuis les
   échantillons décodés, clic sur le spectrogramme pour naviguer. Pas fait : marqueurs automatiques sur des points déterminés
   par l'analyse (clipping, etc.) — écarté du scope actuel à la demande explicite de
   l'utilisateur, à reconsidérer après usage réel du lecteur simple.
6. **Fonctionnalités "audiophile avancé"** ✅ (anticipées depuis V1.0+/V2.0+, ajoutées après
   revue produit du 2026-07-24) :
   - Intégrité FLAC (MD5 embarqué, `decoder.finalize().verify_ok` de symphonia — gratuit).
   - DR14 (Pleasurize Music Foundation), la métrique que cette communauté compare
     publiquement — distincte du crest factor déjà présent.
   - LRA (EBU Tech 3342), companion du LUFS intégré déjà présent.
   - Empreinte d'encodeur lossy dans les tags conteneur (`tags.rs`) — asymétrique dans le
     scoring : une correspondance est une preuve, son absence n'en est pas une.
   - Détection de bit-depth padding / "faux hi-res" (`bit_depth.rs`) — problème distinct
     du transcodage lossy, méthode d'alignement de grille exacte, pas une estimation SNR.
   - Export JSON du rapport (le frontend sérialise, le backend écrit juste le fichier).
   - Binaire CLI (`nyquist-cli`) pour l'usage scripté/batch — partage `analysis.rs` avec
     la commande Tauri, aucune divergence possible entre les deux chemins.
   - Coupure spectrale dans le temps (`cutoff_over_time_hz`), réutilise les données FFT
     déjà calculées pour le spectrogramme.
   - **Explicitement écarté** : détection MQA (profil de risque différent — sujet
     contesté, pas de recherche assez solide pour l'implémenter correctement ; à
     reconsidérer seulement avec un budget de recherche dédié).
7. **V0.4 — Polish UI/UX restant** : historique de sessions, drag & drop de fichier, thème
   clair/sombre explicite (actuellement auto via `prefers-color-scheme` seulement).
8. **V1.0 — Release macOS publique** : `.dmg`, notarization (sinon Gatekeeper bloque l'app
   non signée — anticiper le coût d'un compte développeur Apple), README, doc contributeur.
9. **V1.1+ — Windows** ; **V2.0+ — extensions** : batch/dossier (le CLI couvre déjà une
   bonne partie de ce besoin), comparaison de deux fichiers côte à côte, historique SQLite.
