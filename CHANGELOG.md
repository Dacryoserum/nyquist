# Changelog

All notable changes to this project are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer once
releases start shipping.

## [Unreleased]

### Corrigé

- **Le contenu tonal n'est plus accusé à tort.** `measure_rolloff_steepness` divisait une
  chute en dB fixe par l'écart de fréquence où elle se produit ; sur du contenu tonal (accord
  tenu, piano solo, nappe de synthé, ton de test) cet écart tend vers zéro et la mesure
  saturait à 350 dB/kHz — verdict « probablement transcodé » à 80 % de confiance. La sinusoïde
  de calibration du projet lui-même était classée transcodée. La mesure porte maintenant sur
  une chute bornée de part et d'autre de la coupure, avec deux garde-fous : aucune coupure
  d'encodeur n'existe sous 8 kHz, et l'octave sous la coupure doit être réellement occupée
  (un filtre tronque du contenu large bande ; un accord s'arrête simplement à sa dernière
  partielle).
- **Le silence ne désactive plus la détection.** Le silence numérique se décode en zéros
  exacts, posés sur le plancher dB — lequel se situe *au-dessus* de la bande atténuée réelle
  d'un encodeur lossy. Ces trames noyaient donc la coupure recherchée : 5 s de silence ajoutées
  à un transcodage LAME 128 connu faisaient tomber la pente de 191 à 0 dB/kHz et le verdict de
  « transcodé » à « indéterminé ». Comme presque tout morceau réel commence ou finit dans le
  silence, la détection était en pratique éteinte sur du vrai matériel. Les trames quasi
  silencieuses sont désormais exclues de l'enveloppe, la moyenne se fait en domaine de
  puissance, et le plancher de mesure est distinct du plancher d'affichage.
- **Le clipping était compté à moitié.** Le PCM signé est asymétrique (-32768..=+32767) et les
  décodeurs normalisent par la borne négative : le plein échelle positif arrive à 0,99997 et
  ne franchissait jamais le test `abs() >= 1.0`. Seul le versant négatif était compté —
  vérifié sur une fixture de 1000 échantillons à +32767 et 1000 à -32768, qui rapportait 1000
  au lieu de 2000.
- **Les tags ne peuvent plus renverser la mesure.** Le scan portait sur *toutes* les valeurs
  de tags, commentaires libres compris : un FLAC sans perte dont le commentaire mentionnait
  « iTunes » passait à « probablement transcodé » à 75 %, écrasant un verdict spectral correct.
  `itunes` est retiré de la liste (iTunes/Music est l'un des ripper *lossless* les plus
  répandus, et sa présence n'indique pas quel format il a produit), le scan est limité aux
  clés nommant l'outil d'encodage, et un tag qui contredit un spectre pleine bande donne
  désormais « indéterminé » plutôt qu'une accusation.
- Analyse de profondeur de bits au-delà de 24 bits : rapportée comme non vérifiable au lieu
  d'un résultat faussement confiant (le décodage en `f32` ne porte que 24 bits de mantisse).

### Ajouté

- **Détection du sur-échantillonnage (« faux hi-res » par sample rate)** — `sample_rate.rs`,
  pendant exact de `bit_depth.rs` : un fichier déclarant 96/192 kHz dont le contenu s'arrête
  bien en deçà de la bande passante correspondante. Volontairement séparé du verdict de
  transcodage : ces fichiers sont sans perte de bout en bout, les appeler « transcodés »
  désignerait le mauvais défaut. Signale aussi le taux standard qui suffirait.
- Comptage des paquets illisibles ignorés pendant le décodage (`decode_errors`), remonté
  jusqu'à l'UI et au CLI — les sauter est la bonne récupération, le faire en silence masquait
  exactement le défaut qu'un contrôle d'intégrité doit révéler.
- Corpus étendu de 4 fixtures couvrant les pièges ci-dessus : contenu tonal authentique,
  transcodage entouré de silence, fichier réellement sur-échantillonné, et la combinaison
  lossy + sur-échantillonné. Plus une fixture de calibration à plein échelle sur les deux
  polarités.

### Performance

Mesuré sur un FLAC 96 kHz/24 bits stéréo de 8 minutes, profil release. Valeurs de sortie
**strictement identiques** sur les 16 fichiers de contrôle (JSON complet comparé à pleine
précision) — ces changements réordonnancent le travail, ils ne changent aucun calcul.

- **5,73 s → 3,77 s (-34 %)**. Les quatre étapes post-décodage ne dépendent que du buffer
  décodé et pas les unes des autres : elles tournent maintenant en parallèle, si bien que le
  groupe coûte le temps de la plus lente au lieu de leur somme. Le métrage `ebur128` est en
  outre séparé en deux mètres (`I | LRA` d'un côté, `TRUE_PEAK` de l'autre) qui tournent
  concurremment et font chacun strictement moins de travail que le mètre combiné.
- **1,73 Go → 845 Mo de pic mémoire (-51 %)** : histogramme en une passe dans `bit_depth.rs`
  au lieu de matérialiser tous les échantillons, suppression du buffer mono pleine longueur
  (downmix fait fenêtre par fenêtre), et `frames_db` en un seul buffer contigu au lieu d'une
  allocation par trame (22 500 sur ce fichier).
- Profil release de distribution : LTO complet, `codegen-units = 1`, symboles retirés.
  Binaire 7,1 Mo, `.app` 9,1 Mo, `.dmg` 4,2 Mo.
- `nyquist-cli --timing` affiche le coût par étape, pour que les décisions de perf futures
  restent mesurées plutôt que devinées.

### Déploiement

- **CI GitHub Actions** (`.github/workflows/build.yml`) : `cargo build`/`test`/`clippy -D
  warnings` sur macOS et `npm run check`/`build`, sur chaque PR — l'exigence d'AGENTS.md
  n'était jusqu'ici satisfaite par aucun workflow.
- **Workflow de release** (`release.yml`) : `.dmg` universel (Apple Silicon + Intel) sur tag,
  publié en brouillon. Vérifie d'abord que les versions de `Cargo.toml`, `tauri.conf.json` et
  `package.json` concordent entre elles et avec le tag, et échoue sinon. Les notes de release
  indiquent explicitement que le binaire n'est ni signé ni notarié, avec le contournement
  Gatekeeper — plutôt que de laisser l'utilisateur le découvrir.
- Identifiant de bundle corrigé en `com.nyquist.analyzer` : `tauri build` avertissait que
  `com.nyquist.app` entre en conflit avec l'extension `.app` de macOS. Fait maintenant car
  aucune release n'existe encore — cet identifiant est l'identité de l'app pour macOS.
- Content-Security-Policy définie pour le build de distribution (elle était absente), avec
  une variante de développement séparée pour ne pas bloquer le websocket HMR de Vite.
- Métadonnées de bundle renseignées (catégorie, descriptions, copyright, macOS 10.15
  minimum) et taille de fenêtre minimale, l'UI étant responsive.
- README : instructions de build, commandes de vérification, usage du CLI, et le
  contournement Gatekeeper.

### Modifié

- L'indicateur de chargement est remplacé par l'orbe « composing » de
  [thinking-orbs](https://orbs.jakubantalik.com) (Jakub Antalik, MIT) : une sphère de points
  traversée d'une écharpe ondulante, à la place du cercle tournant. Le paquet est un composant
  React — l'installer aurait imposé React et ReactDOM à une app Svelte pour dessiner un
  indicateur de 64 px, dans un `.dmg` de 4,2 Mo. Seul le peintre de trame, qui ne dépend
  d'aucun framework, a donc été porté ; le canvas, l'horloge et le thème sont écrits en
  Svelte. Le portage est vérifié par comparaison avec l'original : 36 224 appels de dessin
  identiques sur huit instants et les deux thèmes. Voir `THIRD_PARTY_LICENSES.md`.
- Interface repensée autour du verdict : il ouvre la page au lieu d'être enterré sous les
  métadonnées, avec la confiance et les indices attachés. Les problèmes détectés
  (padding de bits, sur-échantillonnage, checksum, paquets corrompus, clipping) apparaissent
  comme constats distincts et n'apparaissent que s'ils existent. Les mesures clés (DR, true
  peak, LUFS, bande passante) sont placées sur leur échelle avec le repère conventionnel,
  pour qu'un chiffre isolé se lise sans connaître les usages. Ajout du glisser-déposer, d'un
  sélecteur de thème clair/sombre, d'un lecteur en barre flottante, et d'un spectrogramme
  agrandi.
- Seuils de `transcode_detect.rs` recalibrés sur la nouvelle échelle de pente (non comparable
  à l'ancienne) : authentique ≤ 12 dB/kHz, transcodages LAME réels 90-94 dB/kHz.
- `bit_depth.rs` : histogramme de bits de poids faible en une passe au lieu de matérialiser
  tous les échantillons en `Vec<i64>` — 1,73 Go → 953 Mo de pic mémoire sur un fichier
  96 kHz/24 bits de 8 minutes.

### Added

- Project scaffolding: agent workflow (`AGENTS.md`, `.claude/`), license (MIT), changelog.
- Tauri + SvelteKit application shell (V0.1): file picker, raw results display.
- Audio decoding via `symphonia` (FLAC, MP3, AAC, ALAC, WAV, OGG).
- Technical file metadata: container, codec, sample rate, bit depth, duration, average
  bitrate, Nyquist frequency.
- Signal analysis: peak, RMS, crest factor, and per-channel clipping count; integrated
  loudness (LUFS) and true peak via `ebur128` (ITU-R BS.1770 / EBU R128).
- Synthetic, reproducible test corpus (`src-tauri/tests/fixtures/corpus/`) with known
  ground truth for future transcode-detection validation: authentic and lossy-transcoded
  (MP3 128/320/V0, AAC 256) fixtures, plus a deliberate false-positive trap (naturally
  treble-poor but genuinely lossless audio).
- Integration tests validating signal analysis against a known-value reference signal and
  against every corpus fixture.
- Spectrogram computation (FFT via `rustfft`, Hann-windowed STFT) and a raw spectral
  cutoff measurement, downsampled and quantized in Rust before transmission (never a dense
  JSON matrix on IPC). Cross-validated against independent ffmpeg measurements on the test
  corpus.
- Redesigned UI: dark theme, card-based dashboard, icon set, canvas-rendered spectrogram
  with an inferno colormap and a "Quiet → Loud" legend.
- Rolloff steepness measurement (dB/kHz) alongside spectral cutoff position — needed
  because cutoff position alone is unreliable on real, dynamic-range-heavy music (see
  `.claude/CONTEXT.md`).
- Transcode likelihood scoring (`transcode_detect.rs`): 3-state verdict (probably
  authentic / probably transcoded / indeterminate) with a bounded confidence score and
  human-readable indicators, never a binary certainty. Validated against the test corpus
  with an explicit false-positive/negative report in `tests/corpus_smoke.rs` (0 false
  positives, 2 documented undetectable cases: LAME V0, AAC 256kbps).
- Audio playback: native `<audio>` element streamed via Tauri's `asset://` protocol
  (seekable, no whole-file JS memory load), with a play/pause/scrub bar and click-to-seek
  directly on the spectrogram.
- FLAC integrity verification: checks the file's own embedded checksum (STREAMINFO MD5)
  against what was actually decoded, via symphonia's built-in decoder verification.
- DR14 (Pleasurize Music Foundation Dynamic Range) — the metric this community compares
  against the public loudness-war database, distinct from the existing crest factor.
  Algorithm verified against the open-source `dr14_t.meter` reference implementation.
- Loudness Range (LRA, EBU Tech 3342) alongside integrated LUFS.
- Encoder tag fingerprint scan (`tags.rs`): flags known lossy-encoder signatures (LAME,
  iTunes, FhG, ...) left over in container tags — a corroborating signal for
  `transcode_assessment`, never used on its own to claim authenticity.
- Bit-depth padding ("fake hi-res") detection (`bit_depth.rs`): flags a file whose real
  content never used the bit depth its container declares (e.g. 16-bit content
  zero-padded into a 24-bit FLAC) — a distinct quality issue from lossy transcoding.
- Spectral cutoff over time: the same cutoff measurement computed per spectrogram time
  window instead of once for the whole file, to catch a transcode that only patches in
  real high-frequency content for part of a track.
- JSON report export from the UI.
- `nyquist-cli`: a headless companion binary for scripting/batch analysis, sharing the
  exact same analysis pipeline as the desktop app.

### Changed

- `AnalysisResult` (backend↔frontend contract) now includes `spectral_analysis`,
  `transcode_assessment`, `dynamic_range`, `encoder_tag_matches`, and `bit_depth_analysis`.
- Removed the unused `rodio` dependency (added speculatively in V0.1, never wired up) in
  favor of the browser-native audio element for playback.
- Extracted the analysis pipeline into a shared `analysis.rs` module, used by both the
  Tauri command and the new CLI binary.
