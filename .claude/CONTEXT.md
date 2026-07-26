# Nyquist — Contexte durable

Pièges spécifiques au dépôt, réutilisables d'une session à l'autre. Garder court :
constats durables uniquement, pas d'historique de session. Supprimer ce qui devient faux.

> Projet démarré le 2026-07-23. V0.1 scaffoldé et compile (décodage + métadonnées +
> RMS/peak/DR/LUFS/true peak, UI liste brute) — voir `git log` pour l'état réel du code,
> ce fichier ne liste que les pièges durables.

## Symphonia 0.6.0 : API différente de la plupart des exemples trouvés en ligne

La majorité des tutoriels/exemples Symphonia sur le web ciblent la 0.5.x. La 0.6.0 (celle
figée dans `Cargo.toml`) a une API notablement différente — vérifié en lisant le code source
réel dans `~/.cargo/registry/src/.../symphonia-0.6.0/examples/` plutôt que de faire
confiance à la mémoire du modèle ou à un exemple 0.5.x trouvé en ligne :

- `Hint` a bougé vers `symphonia::core::formats::probe::Hint` (plus `core::probe::Hint`).
- `get_probe().probe(&hint, mss, fmt_opts, meta_opts)` retourne directement
  `Box<dyn FormatReader>` — plus de wrapper `ProbeResult { format, metadata }`.
- Sélection de piste via `format.default_track(TrackType::Audio)` plutôt qu'un filtrage
  manuel sur `codec_params.codec != CODEC_TYPE_NULL`.
- `track.codec_params` est `Option<CodecParameters>`, un enum : il faut `.audio()` pour
  obtenir `&AudioCodecParameters` (support vidéo/sous-titres dans le même type maintenant).
- Décodeur créé via `get_codecs().make_audio_decoder(...)` (plus `.make(...)` générique).
- `format.next_packet()` retourne `Result<Option<Packet>, Error>` — la fin de flux est
  `Ok(None)`, plus une `Error::IoError(UnexpectedEof)` à intercepter.
- Extraction des échantillons la plus simple : `audio_buf.samples_interleaved()` +
  `audio_buf.copy_to_slice_interleaved(&mut buf)` vers un `Vec<f32>`, plutôt que de gérer
  `SampleBuffer<f32>` à la main.
- Nom du codec/conteneur : `decoder.codec_info().short_name` et `format.format_info().short_name`.

Si une future session doit re-vérifier un détail d'API Symphonia, lire le code source local
(`~/.cargo/registry/src/index.crates.io-*/symphonia-<version>/examples/`) plutôt que de
deviner depuis un exemple 0.5.x trouvé sur le web — les deux API ne sont pas interchangeables.

## ebur128 : API confirmée

`EbuR128::new(channels: u32, rate: u32, mode: Mode)`, `add_frames_planar_f32(&[&[f32]])`,
`loudness_global()` (nécessite `Mode::I`), `true_peak(channel: u32)` (nécessite
`Mode::TRUE_PEAK`, qui implique déjà `SAMPLE_PEAK`). `loudness_global()` renvoie toujours
`Ok`, y compris pour un signal silencieux (retourne alors une valeur non-finie) — filtrer
sur `.is_finite()` avant de sérialiser en JSON (serde_json échoue sur NaN/Infinity).

## Perf : `npm run tauri dev` (profil debug) est trompeusement lent sur du DSP réel

Mesuré le 2026-07-23 sur un FLAC réel (6:52, 24-bit/44.1kHz, 18.2M échantillons/canal,
`decode_file` + `analyze_signal`, `ebur128` en mode `I | TRUE_PEAK`) :

| Build | decode | analyse (RMS/peak/LUFS/true peak) | total |
|---|---|---|---|
| debug (`cargo build` / `tauri dev`) | 23.5s | 81.5s | ~105s |
| release (`cargo build --release`) | 0.47s | 0.86s | 1.3s |

~80x d'écart, valeurs identiques (LUFS/true peak inchangés au chiffre près) — donc pure
lenteur du profil non optimisé (`ebur128` fait beaucoup de filtrage/interpolation
polyphée par échantillon, ce que `-O0` ne vectorise pas), pas un bug ni une boucle
infinie. **Ne pas interpréter un `analyze_file` qui prend >1 min sous `tauri dev` comme un
hang** — c'est attendu sur un fichier long/haute résolution en debug ; comparer contre un
`cargo build --release` avant de creuser plus loin. En production (`tauri build`, release
par défaut), 1.3s pour ~7 minutes de FLAC 24-bit est largement sous le seuil qui
justifierait des événements de progression pour cette étape — l'exigence AGENTS.md
"progression dès le V0.1" reste vraie pour d'éventuels futurs traitements plus lourds
(spectrogramme V0.2), pas retroactivement pour `analyze_file` tel qu'il existe aujourd'hui.

## Spectrogramme : perf et taille de payload mesurées (résolu, plus un risque anticipé)

Sur le même FLAC de référence (6:52, 24-bit/44.1kHz) en release : le calcul spectral
(`spectral::analyze_spectrum`, FFT 4096 pts / hop 2048 / Hann window, downsamplé à 600×300
avant quantification u8 + base64) ajoute **~315ms** au pipeline (total decode+signal+
spectral ≈ 1.6s). Payload IPC : **~240KB** en base64 pour ce fichier. Aucun souci de
perf ni de taille — la stratégie "downsampler + quantifier avant sérialisation" (voir
skill `tauri-ipc-contract`) suffit largement, pas besoin de canal binaire dédié pour
l'instant. Le spectral cutoff brut (`detect_cutoff`, seuil -40dB sous le pic) a été
cross-validé contre des mesures ffmpeg indépendantes (`highpass`+`astats`) sur le corpus :
bonne corrélation sur les coupures nettes (mp3_128 ≈ 16.8kHz mesuré vs ~16kHz ffmpeg,
mp3_320 ≈ 20.2kHz vs ~20.5kHz), confirme correctement l'absence de coupure sur V0/AAC256.
**Sur de la vraie musique (pas le bruit synthétique du corpus), ce cutoff brut peut tomber
assez bas (~8kHz mesuré sur un morceau orchestral réel)** — c'est attendu (l'essentiel de
l'énergie musicale réelle est dans le médium, pas un signe de transcodage) et une bonne
illustration concrète de pourquoi ce chiffre reste explicitement labellisé "raw
measurement, not a verdict" dans l'UI et ne doit jamais servir seul de base à un verdict
en V0.3.

**Mise à jour 2026-07-24** : pipeline complet (decode + signal + DR14 + spectral +
transcode + bit-depth + tags) mesuré à **~2.4s en release** sur le même fichier de
référence — DR14 (itère sur tous les blocs de 3s) et bit-depth (jusqu'à 16 passages
complets sur tous les échantillons, cas défavorable) ajoutent chacun un coût réel mais
individuellement modeste. Toujours sous le seuil qui justifierait de la progression ; à
resurveiller si un futur ajout alourdit encore le pipeline (le CLI, `nyquist-cli`,
partage exactement ce même pipeline via `analysis.rs` — pratique pour rebencher vite :
`time nyquist-cli fichier.flac`).

## Les deux façons dont une mesure de pente spectrale ment (trouvées 2026-07-25, corrigées)

Audit complet du pipeline. Les deux défauts venaient de la *même* fonction
(`measure_rolloff_steepness`) et se compensaient assez pour passer inaperçus sur le corpus
d'origine — qui ne contenait ni contenu tonal, ni silence, ni fichier sur-échantillonné.

1. **Saturation sur contenu tonal → faux positif confiant.** L'ancienne méthode divisait une
   chute fixe (35 dB) par l'écart de fréquence entre deux seuils. Sur du contenu tonal cet
   écart tombe sous la résolution FFT → branche dégénérée `span < 0.1` → 350 dB/kHz →
   « probablement transcodé, 80 % ». **La sinusoïde de `calibration/` était elle-même
   classée transcodée.** Le correctif de 2026-07-24 mentionné plus bas n'avait traité que la
   moitié « aucune coupure » du problème, pas la moitié « contenu étroit ».
2. **Le plancher dB au-dessus de la bande atténuée → faux négatif silencieux.** `linear_to_db`
   plafonnait à `DB_FLOOR = -90`, alors que la bande atténuée d'un encodeur lossy est vers
   -140 dB. Le silence numérique (zéros exacts → -90 dans *toutes* les bandes) relevait donc
   le plancher moyen au-dessus de la coupure à mesurer. 5 s de silence sur un LAME 128 :
   191 → 0 dB/kHz, verdict « transcodé » → « indéterminé ». Presque tout morceau réel
   commence ou finit dans le silence, donc la détection était éteinte en usage réel.

**Leçon réutilisable : ne jamais mesurer une pente en divisant par un écart de fréquence
mesuré** — l'écart peut tendre vers zéro pour des raisons qui n'ont rien à voir avec une
coupure. Mesurer une chute bornée sur une fenêtre *fixe* de part et d'autre. Et garder le
plancher de mesure (`ANALYSIS_FLOOR_DB`, -180) distinct du plancher d'affichage (-90) : les
confondre enterre le signal recherché sous le silence.

Largeur de fenêtre choisie par balayage 300/500/1000 Hz sur le corpus : 300 Hz sépare mieux
(12 vs 135 dB/kHz) mais suppose que la coupure détectée tombe pile sur le filtre, ce qui tient
sur du synthétique et beaucoup moins sur de la vraie musique. 500 Hz garde ~5x de séparation
en moyennant sur ~46 bins.

## Cross-validation : réimplémenter la mesure en numpy avant de conclure

Pour les deux bugs ci-dessus, une réimplémentation Python/numpy fidèle de `spectral.rs`
(même STFT 4096/hop 2048, même downmix, même domaine) a servi de référence indépendante :
elle reproduit exactement les valeurs Rust (191 dB/kHz sur mp3_128) une fois le plancher
traité pareil, ce qui prouve que l'écart observé venait de l'algorithme et non d'un détail
d'implémentation. **Attention au piège rencontré : clamper à -90 dans la référence donnait
des résultats différents** et envoyait sur une fausse piste — reproduire le comportement de
plancher *exactement* avant de comparer.

## Le PCM signé est asymétrique — conséquence sur toute détection de plein échelle

16 bits couvrent -32768..=+32767 et les décodeurs (symphonia inclus) normalisent par la borne
négative : le plein échelle **positif** arrive à 32767/32768 = 0,99997, le négatif à -1,0
exactement. Un test `abs() >= 1.0` ne voit donc *que* le versant négatif — le clipping était
compté à moitié. Seuil correct : un LSB sous 1,0. Vaut pour toute future mesure « au plein
échelle » (inter-sample peaks, détection de saturation).

## Le décodage en f32 plafonne toute analyse de profondeur à 24 bits

La mantisse f32 porte 24 bits significatifs. Au-delà, l'information de poids faible est déjà
perdue au décodage : un fichier 32 bits *paraît* aligné sur une grille plus grossière que
déclarée, ce qui produirait un « faux hi-res » inventé. `bit_depth.rs` répond donc `None`
au-dessus de 24 bits plutôt qu'un résultat confiant. Si un jour l'analyse doit couvrir le
32 bits réel, il faudra un chemin de décodage entier/f64, pas un ajustement de seuil.

## Parallélisme : ce qui a payé, et ce qui a coûté (mesuré 2026-07-25)

Fichier de référence : FLAC 96 kHz/24 bits stéréo, 8 minutes. Profil release avec
`lto = "fat"` + `codegen-units = 1`. Se re-mesure avec `nyquist-cli --timing <fichier>`.

Répartition initiale : decode 1571 ms | **signal 3044 ms** | DR14 140 | spectral 783 |
bit-depth 195 | **total 5734 ms**. Le décodage est séquentiel par nature (bitstream FLAC),
donc le seul levier est l'après-décodage.

Ce qui a marché :
- **Les 4 étapes post-décodage ne dépendent que de `decoded`, pas les unes des autres.**
  Les lancer en `rayon::join` imbriqué met le temps du groupe au niveau du plus lent au
  lieu de la somme.
- **Deux mètres `ebur128` au lieu d'un.** Un mètre `I | LRA | TRUE_PEAK` fait le
  K-weighting *et* le suréchantillonnage polyphase dans une seule traversée séquentielle.
  Séparé en `I | LRA` et `TRUE_PEAK`, chaque mètre fait strictement moins de travail et les
  deux tournent en parallèle. **Ne jamais découper par canal en revanche** : BS.1770 somme
  les puissances pondérées des canaux *avant* le gating, des mètres par canal ne se
  recombinent pas.

Ce qui a coûté et a été annulé :
- **Paralléliser la boucle FFT.** Gain local réel (850 → 620 ms) mais **le total empire**
  (3,67 → 3,88 s) : spectral n'est pas sur le chemin critique, et ses workers volent des
  cœurs à la chaîne de filtres true-peak qui, elle, ne peut pas être découpée. Leçon :
  paralléliser une étape hors chemin critique est une régression, pas une optimisation.
- **Downmix mono à la volée avec une boucle générique sur `Vec<Vec<f32>>`.** Économise bien
  les 184 Mo du buffer mono, mais coûtait plus de CPU qu'il n'en faisait gagner. Récupéré en
  spécialisant les cas 1 et 2 canaux (passe vectorisable) — garder cette spécialisation si
  cette boucle est retouchée.

Résultat : **5734 → ~3770 ms (-34 %)**, valeurs strictement identiques sur les 16 fichiers
de contrôle (vérifié en comparant le JSON complet à pleine précision avant/après — à refaire
pour tout changement de parallélisme, c'est le seul garde-fou contre un réordonnancement qui
change un flottant).

**Variance Apple Silicon** : ~4,3 s observé ponctuellement contre ~3,8 s typique. Les cœurs
P/E ne sont pas distingués par rayon, et un thread critique atterrissant sur un cœur
d'efficacité change le résultat. Toujours mesurer plusieurs fois avant de conclure à une
régression.

## Mémoire : le pipeline charge tout le fichier, et ça se voit sur du hi-res

Mesuré sur un FLAC 96 kHz/24 bits stéréo de 8 minutes : **1,73 Go de pic RSS** avant
correction, dont ~737 Mo pour le seul `Vec<i64>` que `bit_depth.rs` matérialisait sur tous les
échantillons de tous les canaux. Remplacé par un histogramme de bits de poids faible en une
passe (l'alignement sur une grille `2^k` équivaut à « au moins k zéros de poids faible », donc
une passe répond pour tous les candidats à la fois) → **953 Mo**. Puis suppression du buffer
mono pleine longueur (downmix fait fenêtre par fenêtre dans `spectral.rs`) → **845 Mo**, soit
**-51 % par rapport aux 1,73 Go d'origine**. `frames_db` est passé d'un `Vec<Vec<f32>>` (une
allocation par trame, 22 500 sur ce fichier) à un seul buffer contigu — même volume, mais une
allocation et une bien meilleure localité pour les balayages qui parcourent une bande à
travers toutes les trames.

Restent en mémoire simultanée : `channel_samples` (~369 Mo ici) et `frames_db` (~184 Mo). Un
192 kHz ou un long mouvement classique reste un risque : le vrai correctif serait un pipeline
en streaming (analyser par blocs pendant le décodage au lieu de tout charger), ce qui est un
refactor structurel, pas un réglage.

## Position du spectral cutoff seule = trompeuse sur de vraie musique (a motivé le second indicateur)

Sur un extrait réel (Hans Zimmer, passage orchestral calme) : coupure ~7.5kHz **identique
à ±10Hz près, avant et après un vrai passage MP3 128kbps** (source → WAV → MP3 128 →
FLAC). Explication : le contenu naturel était déjà sous la fréquence de coupure de
l'encodeur, qui n'avait donc rien à couper. Testé aussi sur un extrait plus "brillant"
(percussions) : même résultat (~6kHz avant/après, cutoff naturel trop bas pour que 128kbps
morde dessus). **Conclusion : un encodeur lossy ne laisse une empreinte détectable par
cutoff que si la source avait réellement de l'énergie près de la fréquence de coupure de
l'encodeur.** Sur du contenu calme/orchestral, un vrai transcodage peut être totalement
indétectable par cette méthode — un faux négatif silencieux, pas un bug.

A motivé l'ajout de `rolloff_steepness_db_per_khz` dans `spectral.rs` : la **pente** de la
coupure (dB/kHz) sépare bien mieux "filtre d'encodeur" (raide, ~190-270 dB/kHz mesuré sur
LAME réel) de "rolloff naturel" (doux, ~5-10 dB/kHz mesuré aussi bien en synthétique
qu'en musique réelle, transcodée ou non). `transcode_detect.rs` utilise la pente comme
signal principal ; la position ne sert qu'à décrire *où* se situe une coupure déjà
confirmée comme artificielle par sa pente — jamais comme preuve indépendante.

**Piège de calcul rencontré et corrigé** : la première version de `measure_rolloff_steepness`
retournait une valeur élevée (350 dB/kHz, "très raide") pour les fichiers **sans aucune
coupure** (bruit plein spectre, mp3_v0, aac_256) — un bug de division par un span quasi-nul
dans le cas dégénéré "rien à mesurer", confondu avec le cas "transition instantanée". Fixé
en détectant explicitement quand le seuil bas (`STEEPNESS_LOWER_DB`) n'est jamais franchi
avant Nyquist → retourne 0.0 (pas de coupure), pas une fausse valeur "raide". Si ce calcul
est retouché, revalider avec `cargo test corpus_smoke -- --nocapture` et vérifier que les
cas plein-spectre affichent bien `0`, pas une grande valeur.

## Lecture audio : protocole `asset://`, pas de crate audio Rust

Décision (revue par rapport au plan initial qui prévoyait `rodio`) : la lecture utilise
l'élément `<audio>` natif du navigateur + le protocole `asset://` intégré à Tauri, activé
via `tauri = { features = ["protocol-asset"] }` (Cargo.toml) et
`app.security.assetProtocol.enable = true` (tauri.conf.json). `rodio` a été retiré
(ajouté en V0.1, jamais câblé, remplacé par cette approche plus simple).

Mécanisme : `app_handle.asset_protocol_scope().allow_file(&path)` (trait `Manager`,
`tauri::Manager`) autorise dynamiquement exactement le fichier choisi par l'utilisateur —
commande IPC `authorize_playback`, appelée juste avant `convertFileSrc(path)` côté
frontend (`@tauri-apps/api/core`) pour construire l'URL du `<audio src>`. Scope additif,
par fichier ; rien d'autre sur le disque ne devient lisible. Le webview gère
seek/buffering nativement (vraies requêtes range sur le fichier), pas de chargement du
fichier entier en mémoire JS — important pour les gros fichiers 192kHz/24-bit.

API confirmée en lisant le code source local (`~/.cargo/registry/src/.../tauri-2.11.5/src/lib.rs`
et `src/scope/mod.rs`) plutôt que de deviner — `protocol-asset` n'est pas une feature Cargo
par défaut, à activer explicitement.

## DR14 : algorithme vérifié depuis l'implémentation de référence, pas des résumés de forum

Les descriptions du DR14 (Pleasurize Music Foundation) sur les forums audiophiles sont
approximatives ("moyenne du top 20% des RMS") et omettent des détails qui changent le
résultat. Algorithme exact confirmé en lisant le code source de `dr14_t.meter` (Simone
Riva, GPLv3, `github.com/simon-r/dr14_t.meter`, fichiers `compute_dr14.py` +
`audio_math.py`) :

- Blocs de 3s, RMS par bloc = `sqrt(2 * mean(x²))` — **pas** le RMS standard (facteur √2 en
  plus, donc pour un ton stable, RMS_DR14 = peak exactement, pas peak/√2). Conséquence
  contre-intuitive validée : un sinus stationnaire mesure **DR≈0**, pas ~3dB comme le
  laisserait penser un crest factor naïf — voir `tests/calibration.rs`.
- Référence "pic" = le **second plus haut** pic parmi tous les blocs (pas le maximum absolu)
  — évite qu'un seul transitoire/glitch fausse la mesure.
- Référence "RMS fort" = moyenne quadratique (pas arithmétique) du RMS des blocs du top 20%.
- Simplification assumée dans `dynamic_range.rs` : la référence ajoute un correctif `+60`
  échantillons au bloc uniquement à 44100Hz (quirk de cette implémentation précise, pas de
  l'algorithme officiel) — ignoré ici, écart mesuré négligeable (~0.0002 sur la valeur
  finale, jamais assez pour changer l'entier arrondi).

Validation croisée : réimplémentation Python (numpy) du même algorithme, comparée à la
sortie Rust sur plusieurs fixtures — accord à <0.0005 dB près. Si `dynamic_range.rs` est
retouché, revalider de la même façon plutôt que de faire confiance à la mémoire ou à des
résumés secondaires — l'algorithme a plusieurs détails contre-intuitifs (facteur √2,
second pic, moyenne quadratique) faciles à mal reproduire.

## symphonia : vérification d'intégrité et tags, API confirmée

- `AudioDecoderOptions::default().verify(true)` (builder, pas de struct literal —
  `#[non_exhaustive]`) active la vérification embarquée quand le codec la supporte (FLAC :
  MD5 du flux décodé comparé au STREAMINFO). Résultat récupéré **après** la boucle de
  décodage via `decoder.finalize() -> FinalizeResult { verify_ok: Option<bool> }` — pas
  disponible avant la fin du flux. `None` fréquent en pratique même sur de vrais FLAC
  commerciaux : beaucoup de fichiers ré-encodés/ré-tagués ont un MD5 STREAMINFO à zéro
  (non recalculé par l'outil de tag) — distinct de "codec sans support" (MP3/AAC/WAV),
  mais indiscernable du seul champ `verify_ok`. Le frontend doit croiser avec
  `file_info.codec === "flac"` pour distinguer les deux cas dans l'UI.
- Tags (Vorbis comments, ID3) : `format.metadata().current()` juste après le probe (avant
  de lire le moindre packet) suffit pour les formats à métadonnées en en-tête (FLAC, OGG,
  ID3v2) — pas testé/fiable pour les tags en fin de fichier (ID3v1, APEv2), hors scope.
  `RawValue::String` contient un `Arc<String>`, pas un `String` nu — `.to_string()`, pas
  `.clone()`, pour en extraire une valeur owned.

## Bit-depth padding ("faux hi-res") : méthode choisie et pourquoi

Approche retenue dans `bit_depth.rs` : alignement exact des échantillons sur une grille de
quantification plus grossière que la profondeur déclarée, **pas** une estimation de bruit
de plancher/SNR théorique (`6.02*N+1.76 dB`) — cette dernière demanderait un passage calme
fiable (peu robuste sur un master brickwallé, justement le genre de fichier où ce indicateur
doit aussi fonctionner) et une constante non citable depuis une norme ITU/EBU comme pour le
LUFS. Limite assumée : un fichier correctement dithéré avant le padding échappe à cette
détection (le dither ajoute du bruit sous-LSB qui casse l'alignement exact) — accepté
sciemment : ça garde le détecteur 100% sans faux positif (validé sur tout le corpus), au
prix de rater les cas proprement dithérés (rares en pratique pour un faux hi-res paresseux,
qui est justement le cas réel visé). Perf mesurée : ~326ms sur un FLAC réel de 6:52/24-bit
(pire cas : aucun candidat ne correspond, 16 passages complets sur ~36M échantillons).

## macOS Gatekeeper (risque anticipé, pas encore rencontré)

Un `.dmg` non signé/notarié déclenche un avertissement bloquant à l'ouverture
(« développeur non identifié »). Pas de solution gratuite — soit un compte développeur
Apple (99$/an) pour notariser, soit documenter clairement le contournement clic-droit-ouvrir
pour les early adopters en attendant. Voir la skill `release-packaging`.

## Environnement local

- **Rust installé via rustup le 2026-07-23** (pas présent avant) : `rustc`/`cargo` 1.97.1.
  `rustup` ajoute `. "$HOME/.cargo/env"` à `~/.bash_profile`, pas à la config zsh (le shell
  par défaut du poste) — ajouté manuellement à `~/.zshenv` (sourcé pour les shells
  interactifs et non-interactifs). Si une session a `cargo: command not found`, sourcer
  `"$HOME/.cargo/env"` explicitement.
- **Node 18.20.3** (nvm) — suffisant pour Vite 6 / SvelteKit 2 / Svelte 5 utilisés ici.
- Scaffold initial généré via `npm create tauri-app@latest . -- -m npm -t svelte-ts
  --identifier com.nyquist.app -y -f` (le `-f` force l'écriture dans un dossier non vide,
  nécessaire ici car `AGENTS.md`/`.claude/`/etc. existaient déjà). **L'identifiant a depuis
  été changé en `com.nyquist.analyzer`** : `tauri build` avertit qu'un identifiant finissant
  par `.app` entre en conflit avec l'extension de bundle macOS. Corrigé avant toute release,
  donc sans dette — mais ne pas le re-changer après la première release publique, c'est
  l'identité de l'app pour macOS (préférences, permissions, mises à jour).

