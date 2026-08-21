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
de transcodage 3 états, lecture audio native avec clic-pour-naviguer, export JSON, et un
binaire CLI (`nyquist-cli`) pour l'usage scripté/batch. UI dashboard (thème sombre chaud,
cartes, icônes, canvas spectrogramme + dégradé inferno) — validée visuellement et
fonctionnellement en conditions réelles par l'utilisateur.

**Le scoring V0.3 (transcodage lossy) reste délibérément conservateur** — signal principal :
pente de rolloff (voir `transcode_detect.rs`), confiance plafonnée (≤0.9 avec empreinte de
tag corroborante, ≤0.8 spectral seul, ≤0.6 authentique). Cross-validé contre des mesures
ffmpeg indépendantes et, ponctuellement hors corpus versionné, contre de vrais FLACs
commerciaux retranscodés (a révélé que la *position* seule du cutoff est trompeuse sur de la
musique réelle — voir `.claude/CONTEXT.md`). Rapport FP/FN sur le corpus
(`tests/corpus_smoke.rs`) : **20 fixtures, 0 faux positif, 0 faux négatif inattendu, 5
échecs documentés**.

⚠️ **Le point aveugle n'est pas un simple raté : c'est une affirmation fausse.** LAME V0 et
AAC 256 ne coupent pas, donc ils sortent en « probablement authentique » à 60 % — l'outil
cautionne le faux au lieu de dire « je ne sais pas ». L'AAC 128 s'échappe aussi sur du
matériau non-stationnaire (27 dB/kHz, sous le seuil de 40) alors qu'il est attrapé sans
peine sur du bruit plat : ce qui décide, c'est le matériau, pas le débit. Quatre pistes ont
été prototypées pour combler ça (trous spectraux, grille de trames, stabilité de coupure,
stéréo joint) — **aucune ne sépare un encodage transparent d'un lossless**, et l'une accuse
un piano « in the box » plus fort qu'un vrai transcodage. Mesures consignées dans
`tests/fixtures/corpus/README.md` : les relire avant de retenter.

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
│   │   ├── stereo.rs           # ✅ Corrélation L/R, side/mid global et par bande, dual-mono
│   │   ├── transcode_detect.rs # ✅ Scoring 3 états, voir la skill dédiée + module docs
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
| Lecture audio | **pas de crate Rust** | Décision revue : `<audio>` natif du navigateur + protocole `asset://` de Tauri (`tauri = { features = ["protocol-asset"] }`), pas `rodio`. Le webview gère seek/buffering nativement sans charger le fichier entier en mémoire JS ; `rodio` a été retiré du Cargo.toml (ajouté en V0.1, jamais utilisé). Voir `commands::authorize_playback`. |
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
voir la skill `tauri-ipc-contract`) + `transcode_assessment` (verdict 3 états +
`confidence_score` + `indicators[]` : chaque indice porte la prose anglaise du backend
(`message`, ce que voient le CLI et le JSON exporté) **et** son `code` + ses mesures brutes,
pour que l'UI recompose la phrase en français — ajouter une variante à `IndicatorDetail`
oblige à traduire dans `i18n.svelte.ts`, sinon `npm run check` échoue) +
`encoder_tag_matches` + `bit_depth_analysis` + `stereo_analysis` (corrélation L/R, side/mid
par bande, dual-mono exact — `null` si le fichier n'est pas exactement stéréo).
Payload mesuré ~240KB pour un FLAC de 6:52, calcul total ~2.4s en release (voir CONTEXT.md).
Deux autres commandes IPC : `authorize_playback(path)` (juste avant `convertFileSrc(path)`
côté frontend pour la lecture) et `export_report(path, json)` (le frontend sérialise et
appelle `save()` lui-même, le backend ne fait qu'écrire le fichier).

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
4. **V0.3 — Détection de transcodage** ✅ (premier slice) : scoring 3 états basé sur la
   pente de rolloff + empreinte de tags encodeur, indices explicites, validé contre le
   corpus (0 FP, 2 échecs documentés). Point aveugle restant : LAME V0/AAC256, aucun
   lowpass à détecter par cette méthode — reste ouvert (pas de nouvel indicateur qui le
   couvrirait spécifiquement identifié pour l'instant).
5. **Lecture audio** ✅ (anticipée depuis V0.4) : `<audio>` natif + `asset://`, clic sur le
   spectrogramme pour naviguer. Pas fait : marqueurs automatiques sur des points déterminés
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
