# Changelog

All notable changes to this project are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer once
releases start shipping.

## [Unreleased]

### Modifié

- **Builds de développement optimisés.** Le balayage MDCT s'effondre sans optimisation : une
  piste de 5 minutes prenait **55 s** sous `tauri dev` contre moins d'une seconde en release,
  ce qui rendait l'app de dev inutilisable pour son propre usage et le test de corpus
  interminable. Les dépendances sont désormais compilées en `opt-level = 2` et notre propre
  crate en `opt-level = 1` (assez bas pour garder des backtraces lisibles). Même piste :
  **1,2 s**. Le test de corpus passe de 207 s à 5,8 s. Aucun effet sur le binaire de release.


### Ajouté

- **Détection de la grille MDCT — le point aveugle AAC est fermé** (`mdct_grid.rs`). La MDCT
  est inversible (TDAC) : en ré-analysant le signal décodé avec la taille de fenêtre et le
  **décalage de trame** exacts d'un encodeur AAC, on retrouve ses propres coefficients
  quantifiés, dont ceux qu'il a mis à zéro. Un fichier sans perte n'a aucun alignement de ce
  genre. C'est un test *structurel*, pas statistique : il ne lit pas l'enveloppe spectrale,
  donc il n'est pas aveugle aux encodages transparents. Sur le corpus : 12 fixtures
  authentiques à z ≤ 4,7 contre 79, 132 et 215 pour les trois transcodages AAC, tous les
  trois d'accord sur le décalage 960. Le rapport passe de **5 ratés documentés à 2** — sans
  aucun faux positif. Le MP3 n'est pas couvert et ne peut pas l'être ainsi : son banc de
  filtres hybride n'est pas une MDCT simple. LAME V0 reste le point aveugle.
- **Visualisation de la grille.** Le balayage complet des 1024 décalages est transmis à
  l'interface et dessiné tel quel : un fichier sans perte donne un relief bas et irrégulier,
  un encodeur AAC un plancher plat surmonté d'un pic unique. La preuve est lisible sans
  qu'on ait à expliquer un seuil.
- **Comparaison de deux fichiers.** Un « + » discret dans la barre du haut ouvre un second
  fichier ; les deux verdicts, un tableau de mesures aligné et les deux spectrogrammes se
  lisent côte à côte. Là où « mieux » est sans ambiguïté (bande passante, plage dynamique,
  écrêtage, grille MDCT), le côté qui mène est signalé — ailleurs les valeurs sont posées
  côte à côte sans arbitrage, faute de base pour pondérer un spectre plus large contre un
  master plus fort.
- **Choix de la palette du spectrogramme** : inferno (défaut), viridis, glace, monochrome.
  Quatre pastilles sous la légende, persistées localement. Les quatre sont perceptuellement
  ordonnées — la clarté croît avec l'intensité — pour que l'image ne fabrique pas de
  contours absents des données.
- **Confiance affinée sur les fichiers authentiques.** Deux éléments de preuve *positive*
  s'ajoutent désormais, chacun visible dans la liste d'indices : un balayage de grille propre
  (qui écarte l'AAC) et du contenu au-dessus du plafond de 22,05 kHz d'une source à la
  fréquence du CD (qui écarte le chemin de transcodage le plus courant). Volontairement
  petits (+0,05 chacun, plafond 0,70) : le point aveugle MP3 reste ouvert. Le bonus hi-res
  exige que le fichier occupe réellement sa bande passante déclarée, sinon la bande de
  transition d'un rééchantillonneur le déclencherait sur un fichier issu d'un CD.

### Corrigé

- La légende du spectrogramme (« Quiet »/« Loud ») était restée en anglais.

## [0.3.0] - 2026-08-21

### Ajouté

- **Analyse de l'image stéréo** (`stereo.rs`) : corrélation L/R, rapport side/mid global et
  par bande (graves/médiums/aigus), détection exacte du dual-mono (canaux identiques au bit
  près) et alerte de compatibilité mono quand la corrélation est négative. C'est de
  l'information sur le fichier, **pas** un indice de transcodage — voir ci-dessous.
- **Détail spectral exposé** : niveaux moyens par bande de fréquence (relatifs à la bande la
  plus forte), profondeur du stopband, et stabilité de la coupure dans le temps. La table de
  bandes rend le verdict inspectable : un mur d'encodeur y apparaît comme une chute brutale
  entre deux bandes voisines, un master sombre comme une pente régulière.
- **Corpus : matériau non-stationnaire et vraie stéréo.** Sept fixtures, dont une source
  bâtie sur deux graines de bruit décorrélées avec passages calmes, transitoires et contenu
  tonal, plus ses transcodages MP3 128/V0 et AAC 128/256, plus deux pièges à faux positifs.
  Le corpus précédent était intégralement du bruit stationnaire en dual-mono : tous les
  seuils du projet avaient été calés sur un matériau qui n'exerce pas les phénomènes
  recherchés.

### Corrigé

- **Faux edge près de Nyquist.** Le balayage allait jusqu'à `nyquist − sonde`, si bien qu'un
  candidat pouvait se poser assez haut pour que le garde-fou « la chute se maintient jusqu'à
  Nyquist » ne mesure plus qu'une bande de quelques centaines de Hz — à ce stade il ne prouve
  plus rien, un spectre qui descend encore le franchit. Un fichier réellement lossless avec
  un lowpass de mastering doux à 15 kHz rapportait une coupure fantôme à 21,5 kHz. Le
  balayage réserve maintenant 1 kHz de stopband à mesurer. Effet sur le corpus :
  `authentic_44k_lowpass_naturally.flac` passe d'« indéterminé » à « probablement
  authentique », aucune régression ailleurs.
- **La liste d'indices du verdict restait en anglais.** C'est le texte le plus lu de
  l'application — la justification affichée sous le verdict — et il était écrit en dur dans
  `transcode_detect.rs`, donc hors de portée du catalogue de traduction ajouté en 0.2.0 :
  une interface française annonçait « Probablement transcodé » puis expliquait pourquoi en
  anglais. Le backend émet désormais chaque indice sous forme de code plus ses mesures, et
  l'interface recompose la phrase dans la langue affichée. Le `nyquist-cli` et les rapports
  JSON exportés gardent la prose anglaise mot pour mot : un rapport se lit et se compare de
  la même façon quelle que soit la langue dans laquelle il a été produit.
- Traduits également : les libellés de filtre des fenêtres système « ouvrir » et
  « enregistrer », la profondeur de bits (« 24 bits » et non « 24-bit »), le nombre de canaux
  et l'unité de taille de fichier (« Mo » et non « MB »).

### Modifié — ce que le corpus révèle sur la détection

- **Le point aveugle est plus large que documenté, et pire que « raté ».** Sur du matériau
  non-stationnaire, l'AAC 128 s'échappe aussi (coupure à 18,3 kHz mais seulement 27 dB/kHz,
  sous le seuil de 40) alors que le même encodeur sur du bruit plat mesure ~106 dB/kHz : ce
  qui décide, c'est le matériau, pas le débit. Et pour LAME V0 comme pour l'AAC 256, le
  verdict n'est pas « je ne sais pas » mais « probablement authentique » à 60 % — l'outil
  cautionne le faux. `corpus/README.md` réclamait « indéterminé, jamais un authentique
  confiant » depuis l'écriture du corpus ; c'est désormais mesuré et affirmé par les tests
  plutôt que noté en passant. Le rapport FP/FN passe de 2 à 5 ratés documentés, sans aucun
  faux positif.
- **Quatre pistes prototypées contre le point aveugle, aucune livrée.** Trous spectraux
  (attrape LAME V0 avec une marge nette, mais accuse un piano « in the box » *plus fort*
  qu'un vrai transcodage), grille de trames du codec (aucun signal — le recouvrement TDAC
  lisse la périodicité), stabilité de la coupure (résultat inversé : un lowpass de mastering
  est aussi fixe que celui d'un codec), effondrement stéréo joint (aucune séparation). Les
  mesures sont consignées dans `corpus/README.md` pour que la prochaine tentative ne les
  refasse pas. Les deux dernières sont exposées comme information, jamais comme score.


### Modifié

- **Contrat IPC (rupture)** : `transcode_assessment.indicators` était un tableau de chaînes,
  c'est maintenant un tableau d'objets `{ message, code, …mesures }`. Les scripts qui lisent
  la sortie `--json` du CLI doivent utiliser `.message` là où ils lisaient la chaîne
  directement. Un nouvel indice ajouté côté Rust sans sa traduction fait désormais échouer
  `npm run check`, plutôt que d'atteindre silencieusement l'interface en anglais.

## [0.2.0] - 2026-08-20

First tagged release. Everything below predates a public version number — see
`.claude/CLAUDE.md` for the project's actual phase (V0.1 → V0.3 first slices, plus audio
playback and the "advanced audiophile" feature set, all already in place before this tag).

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
- **Interface en français par défaut**, avec un bouton discret en haut à droite de la
  barre du haut pour basculer en anglais (persisté en local, aucune requête réseau). Tout
  le texte de l'interface — dashboard, constats, lecteur, spectrogramme — passe par un
  dictionnaire partagé (`src/lib/i18n.svelte.ts`) ; les nombres suivent aussi la
  convention décimale de la langue active (virgule en français). Les messages d'erreur
  bruts renvoyés par le backend restent non traduits (texte technique de diagnostic).
- **Contrôle de volume sur le lecteur audio** : bouton muet + curseur, à côté de la barre
  de progression existante, avec le volume choisi mémorisé d'une session à l'autre.

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

- **Icône de l'application** remplacée : le logo par défaut généré par `npm create
  tauri-app` (cercles teal/jaune, sans rapport avec l'app) laisse place à un dessin dans le
  thème réel de l'interface — carré arrondi sombre (`--bg`) avec des barres de spectre
  colorées par le dégradé inferno de `src/lib/colormap.ts`, la seule vraie surface de
  couleur de l'app. Régénérée pour toutes les plateformes via `tauri icon`.
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
