> ## ⚠️ Document périmé — ne pas utiliser comme référence
>
> Cet audit (V1) décrit un état du dépôt qui n'existe plus. Il cite `media_protocol.rs`,
> supprimé depuis ; il décrit un blocage de build lié à la feature Cargo `protocol-asset`,
> corrigée depuis ; et il affirme que les tests ne passent pas, alors qu'ils passent.
>
> **Référence à jour : `AUDIT_CORRECTION_EXECUTABLE.md`** (V3), qui reprend et re-vérifie
> les constats encore valides. `AUDIT_V2_PLAN_CORRECTION.md` est conservé pour l'historique.
> Ce fichier-ci n'est gardé que pour la traçabilité des constats d'origine.

# Audit complet de Nyquist

Date : 2026-08-25

## Portée

Audit statique du dépôt, complété par la lecture des tests, du corpus, des dépendances locales
et des vérifications de build disponibles. L'objectif était de repérer les bugs, les risques de
mesures fausses, les limites de l'heuristique de transcodage, les incohérences d'interface et
les améliorations structurantes possibles.

Le rapport ne propose pas de masquer les limites du détecteur. Pour cette application, un faux
positif accuse injustement un master légitime et un faux négatif peut cautionner un faux fichier
lossless. Les deux doivent donc rester visibles dans les résultats.

## Etat du worktree

Des modifications sont apparues dans le worktree pendant l'audit et ne viennent pas de moi :

- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tauri.conf.json`
- `src/lib/api.ts`
- `src/routes/+page.svelte`
- `src-tauri/src/media_protocol.rs` (nouveau fichier)

Elles concernent notamment le remplacement du protocole `asset://` par un protocole média
personnalisé. Elles sont conservées telles quelles. Les remarques qui suivent portent sur l'état
du dépôt observé, y compris ces changements.

## Résumé exécutif

Le projet possède déjà une bonne base : pipeline partagé GUI/CLI, décodage Symphonia, métriques
LUFS/true peak via `ebur128`, corpus reproductible, downsampling du spectrogramme avant IPC et
indicateurs explicables.

Les améliorations les plus importantes ne sont pas de nouveaux seuils. Il faut d'abord :

1. Empêcher un verdict normal lorsqu'une partie du fichier n'a pas été décodée.
2. Arrêter de présenter l'absence de coupure comme une preuve d'authenticité.
3. Distinguer une bande passante mesurée d'une valeur de repli égale à Nyquist.
4. Ajouter une qualité de mesure et une confiance réellement interprétable.
5. Elargir le corpus avant de modifier les seuils.
6. Corriger les métriques multicanales, le clipping, le bit depth et le true peak hi-res.
7. Passer progressivement à un pipeline streaming pour éviter les pics mémoire.
8. Corriger les courses d'analyse et les erreurs silencieuses de l'interface.

## Résultats des vérifications

### Vérifications réussies sur le snapshot initial

- `cargo test --locked --release -- --nocapture` : réussi.
- `cargo clippy --locked --all-targets -- -D warnings` : réussi.
- `npm run check` : réussi, 0 erreur et 0 warning.
- `npm run build` : réussi.
- Corpus : 20 fixtures, 0 faux positif, 0 faux négatif inattendu et 2 ratés documentés.

Le corpus initial affichait notamment :

- `transcoded_mp3_128_44k.flac` : transcodé, confiance 80 %.
- `transcoded_mp3_320_44k.flac` : transcodé, confiance 80 %.
- `transcoded_aac_256_44k.flac` : transcodé grâce à la grille MDCT, confiance 90 %.
- `transcoded_mp3_v0_44k.flac` : classé probablement authentique, confiance 65 %.
- `transcoded_dynamic_mp3_v0_44k.flac` : classé probablement authentique, confiance 65 %.

Ces deux derniers cas sont connus, mais restent le principal problème produit : l'application
ne se contente pas de dire « je n'ai pas détecté », elle dit actuellement « probablement
authentique ».

### Vérifications sur l'état actuel modifié

- `npm run check` : réussi.
- `npm run build` : réussi.
- `cargo test --locked --release -- --nocapture` : bloqué à la compilation.
- `cargo clippy --locked --all-targets -- -D warnings` : bloqué à la compilation.
- `cargo fmt --all -- --check` : échoue déjà sur le formatage Rust existant.

Le blocage backend actuel est :

```text
The `tauri` dependency features on the `Cargo.toml` file does not match the allowlist
defined under `tauri.conf.json`.
Please run `tauri dev` or `tauri build` or remove the `protocol-asset` feature.
```

Cause : `src-tauri/Cargo.toml:28` active encore `protocol-asset`, tandis que
`src-tauri/tauri.conf.json:25-27` le désactive. Si le protocole personnalisé remplace bien
`asset://`, il faut probablement supprimer la feature Cargo. Sinon il faut réactiver et garder
la configuration asset cohérente.

# 1. Résultats et détection de transcodage

## P0 - Verdict produit trop affirmatif en absence d'indice

**Fichiers :** `src-tauri/src/transcode_detect.rs:505-541`,
`src-tauri/tests/fixtures/corpus/README.md:51-65,82-88`.

### Constat

Quand aucune coupure d'encodeur n'est trouvée, le code retourne `ProbablyAuthentic` avec une
confiance de 0,60 à 0,70. Or le corpus montre que LAME V0 peut être un transcodage réel sans
laisser de coupure exploitable.

### Impact

Le produit valide activement un faux lossless dans le cas le plus important du point aveugle
MP3. Cela peut donner à l'utilisateur une fausse assurance.

### Piste de correction

- Faire de `Indeterminate` le résultat par défaut quand il n'existe aucune preuve positive.
- Garder un indicateur « aucune coupure détectée » et un indicateur « encodages transparents non
  détectables ».
- Réserver `ProbablyAuthentic` à une combinaison de preuves positives et indépendantes.
- Présenter le résultat comme « aucun indice de transcodage détecté » plutôt que « authentique ».
- Garder le bonus au-dessus de 22,05 kHz comme réduction d'un scénario possible, pas comme preuve
  historique d'origine.

### Validation attendue

Le corpus doit continuer à avoir 0 faux positif, tandis que les deux LAME V0 doivent devenir
`Indeterminate` au lieu de `ProbablyAuthentic`.

## P0 - Verdict calculé sur un décodage incomplet

**Fichiers :** `src-tauri/src/decode.rs:89-110`, `src-tauri/src/analysis.rs:148-160`.

### Constat

Les erreurs `IoError` et `DecodeError` sont comptées puis ignorées. `ResetRequired` arrête le
décodage sans incrémenter `decode_errors`. L'analyse et le verdict continuent malgré tout.

### Impact

Un fichier tronqué, corrompu ou partiellement lisible peut recevoir un verdict spectral ou MDCT
normal sur un signal incomplet. La durée peut également être fausse.

### Piste de correction

- Ajouter un état explicite `decode_complete` ou `audio_coverage` dans le contrat.
- Marquer `ResetRequired` et toute erreur sautée comme décodage incomplet.
- Forcer l'assessment de transcodage à `Indeterminate` et confiance nulle ou très basse si le
  signal est incomplet.
- Conserver les mesures disponibles, mais les afficher comme mesures d'un extrait survivant.
- Ajouter un indicateur backend explicite pour expliquer le blocage du verdict.

### Tests attendus

- FLAC tronqué avec et sans MD5.
- Fichier contenant un paquet décodable après une erreur.
- OGG chaîné provoquant `ResetRequired`.
- Vérification que la longueur déclarée et la longueur réellement analysée sont cohérentes.

## P1 - Bande passante confondue avec Nyquist

**Fichiers :** `src-tauri/src/spectral.rs:276-280`, `src-tauri/src/sample_rate.rs:63-88`,
`src/lib/api.ts:90-106`, `src/routes/+page.svelte:451-462`.

### Constat

Lorsque `find_spectral_edge` ne trouve rien, `spectral_cutoff_hz` vaut Nyquist. Un fichier
bass-only affiche donc une bande passante de 22,05 kHz et 100 %, alors que son contenu réel est
limité aux graves. Ce comportement a été observé dans la fixture `authentic_bass_only_44k`.

### Impact

- La fiche technique affirme une bande passante qui n'a pas été mesurée.
- La détection de faux hi-res peut être trompée.
- Les comparaisons entre fichiers deviennent fausses.
- Le graphe et le texte ne racontent pas la même chose sur du contenu naturellement sombre.

### Piste de correction

- Remplacer `spectral_cutoff_hz: f64` par `Option<f64>` si aucune limite n'est mesurable.
- Ou ajouter `spectral_cutoff_measured: bool` et une valeur de repli séparée.
- Séparer trois notions : limite de contenu observée, edge de filtre détectée et Nyquist déclaré.
- Faire que `sample_rate_analysis` ignore une mesure non établie au lieu de la prendre pour 100 %.
- Afficher « aucune limite mesurable » plutôt que « Nyquist » dans ce cas.

## P1 - Meilleur candidat spectral choisi avant les gates

**Fichier :** `src-tauri/src/spectral.rs:557-588`.

### Constat

Le balayage conserve uniquement la chute la plus forte, puis applique l'occupation de bande et
la profondeur du stopband. Si cette chute est un notch musical rejeté par les gates, le code ne
teste pas le deuxième candidat, qui pourrait être une vraie coupure de codec.

### Impact

Faux négatifs possibles sur de la musique réelle contenant plusieurs accidents spectraux.

### Piste de correction

- Conserver les meilleurs candidats au lieu d'un seul.
- Trier par chute puis appliquer les gates à chacun.
- Retourner le meilleur candidat valide.
- Ajouter un score de séparation entre le meilleur candidat accepté et les autres.

### Test attendu

Construire un spectre synthétique avec un notch plus profond mais rejeté, puis une coupure codec
moins profonde mais valide. La seconde doit être sélectionnée.

## P1 - Données temporelles calculées mais absentes du verdict

**Fichiers :** `src-tauri/src/spectral.rs:136-153,282-286`,
`src-tauri/src/transcode_detect.rs:486-513`.

### Constat

`cutoff_over_time_hz` est calculé et affiché, mais le scoring utilise uniquement le spectre moyen
global. Un transcodage présent sur une partie du fichier peut être dilué par le reste du morceau.

### Impact

Le commentaire promet de détecter un transcodage local, mais le verdict ne l'utilise pas.

### Piste de correction

- Calculer un edge par fenêtre active avec une référence locale et une référence globale.
- Ajouter `edge_presence_ratio`, `edge_steepness_percentile` et `active_window_count`.
- Scorer la proportion de fenêtres montrant un edge cohérent, avec un minimum d'observations.
- Si la donnée reste purement descriptive, retirer de la documentation toute promesse de scoring.

## P1 - Confiance non calibrée et corpus trop homogène

**Fichiers :** `src-tauri/src/transcode_detect.rs:67-124`,
`src-tauri/tests/fixtures/corpus/README.md:3-8,67-76`,
`src/routes/+page.svelte:411-415`.

### Constat

Les valeurs 0,25, 0,60, 0,75, 0,80, 0,90 et 0,95 sont des scores heuristiques réglés sur un
petit corpus, pas des probabilités. L'interface les affiche pourtant comme des pourcentages.

Le corpus est majoritairement synthétique, contient beaucoup de bruit stationnaire et repose
principalement sur un encodeur AAC Apple pour la partie MDCT.

### Impact

Un utilisateur peut lire « 90 % » comme une probabilité statistique validée. Le score donne une
précision apparente que les données ne permettent pas de justifier.

### Piste de correction

- Remplacer « confiance » par « force des indices » tant qu'il n'existe pas de calibration.
- Afficher `faible`, `modérée`, `forte` ou une barre sans symbole `%`.
- Conserver le score brut dans le JSON technique si nécessaire.
- Créer un corpus de réglage et un corpus de validation séparé.
- Mesurer précision, rappel, faux positifs, faux négatifs et calibration sur le corpus de test.
- Rapporter la taille et la composition du corpus avec les résultats.

## P1 - La grille MDCT AAC est plus limitée que son libellé

**Fichiers :** `src-tauri/src/mdct_grid.rs:24-43,51-74,126-186`,
`src/lib/i18n.svelte.ts:288-291,429-440`.

### Constat

L'algorithme suppose une MDCT AAC longue de 1024 échantillons, une fenêtre sine et analyse le
premier canal uniquement. Les encodeurs AAC peuvent utiliser des short blocks, des séquences
de block switching, d'autres fenêtres, des layouts multicanaux et des traitements ultérieurs.

### Impact

Un AAC transcodé peut être déclaré « grille absente » ou « non analysable », alors que le texte
semble couvrir les fichiers AAC en général.

Le premier canal peut aussi être silencieux alors qu'un autre canal contient tout le signal.

### Piste de correction

- Choisir le canal actif le plus informatif au lieu du premier canal.
- Retourner le niveau d'énergie et le nombre de canaux examinés.
- Tester plusieurs tailles et séquences de fenêtres seulement si elles sont validées.
- Limiter explicitement le message à « AAC long-block compatible » tant que le périmètre n'est
  pas élargi.
- Ajouter un état `not_applicable` ou `unsupported_profile`, distinct de `clear`.

### Tests attendus

- AAC avec transitoires provoquant des short blocks.
- Plusieurs encodeurs AAC.
- AAC ré-échantillonné avant conversion lossless.
- Premier canal silencieux et second canal actif.
- Fenêtre KBD.

## P1 - Limite basse fixe de 8 kHz

**Fichier :** `src-tauri/src/spectral.rs:73-79`.

### Constat

Les coupures sous 8 kHz sont volontairement ignorées comme contenu naturellement étroit.

### Impact

Les encodages à très bas débit ou certains codecs agressifs peuvent être invisibles, même si la
coupure est effectivement due à l'encodeur.

### Piste de correction

Ne pas simplement baisser le seuil. Ajouter un classifieur de contexte basé sur l'occupation
broadband, la durée de l'edge et la cohérence temporelle. Tant que ce travail n'est pas validé,
documenter clairement cette limite et garder le résultat indéterminé.

## P2 - Détection de faux hi-res insuffisamment protégée contre les masters naturellement sombres

**Fichiers :** `src-tauri/src/sample_rate.rs:33-44,63-88`,
`src-tauri/tests/corpus_smoke.rs:493-537`.

### Constat

Un fichier authentique en 96 ou 192 kHz mais naturellement limité dans les aigus peut franchir
le seuil `MIN_BANDWIDTH_RATIO` et être marqué `likely_upsampled`. Le corpus possède un master
naturellement sombre en 44,1 kHz, mais pas de vrai master hi-res naturellement sombre.

La valeur `sufficient_sample_rate_hz` applique en plus une tolérance de 0,9 qui n'est pas évidente
dans la formulation « taux suffisant » affichée par l'interface.

### Piste de correction

- Ajouter des fixtures authentiques hi-res à bande naturellement limitée.
- Retourner un statut de mesure lorsque la bande passante a été réellement mesurée.
- Afficher « compatible avec un sur-échantillonnage » plutôt que « fréquence gonflée ».
- Documenter la tolérance appliquée avant de choisir le taux suffisant.
- Ne pas faire monter la confiance sans preuve d'une transition de ré-échantillonnage.

## P1 - Résultats de tags incomplets ou comptés de travers

**Fichiers :** `src-tauri/src/tags.rs:45-70`,
`src-tauri/src/transcode_detect.rs:226-233,442-450`.

### Constat

- Seuls les tags disponibles immédiatement après le probe sont lus.
- Les tags de fin de fichier, comme ID3v1 ou APEv2, ne sont pas couverts.
- Un même tag contenant deux motifs produit deux matches.
- `additional_matches` est ensuite présenté comme un nombre de tags supplémentaires.

### Impact

Le rapport peut sous-compter la présence de traces et afficher une formulation incorrecte.

### Piste de correction

- Dédupliquer par couple `(tag_key, tag_value)` ou distinguer `matching_patterns` de
  `matching_tags`.
- Afficher la limite de couverture des métadonnées.
- Ajouter des tests pour plusieurs motifs dans un tag, plusieurs tags et les tags de fin de
  fichier.
- Garder l'absence de tag comme absence d'information, jamais comme preuve d'authenticité.

## P2 - Verdict `declared_lossy` incohérent avec la documentation

**Fichiers :** `src-tauri/src/transcode_detect.rs:147-163,353-363`,
`src-tauri/cli/src/main.rs:187`, `src/lib/api.ts:110-117`, `README.md:53-57`.

### Constat

Le code possède quatre états, dont `declared_lossy`, alors que plusieurs documents parlent encore
de trois états. Le backend renvoie une confiance de `1.0`, l'interface masque le pourcentage et
le CLI affiche `100 %`.

### Piste de correction

- Documenter explicitement quatre états.
- Remplacer la confiance par `null` ou une notion `confidence_kind: declared | inferred`.
- Faire afficher au CLI la même sémantique que l'interface.
- Ajouter un golden JSON et des tests de rendu pour chaque état.

## P2 - Liste de codecs déclarés avec perte non exhaustive

**Fichier :** `src-tauri/src/transcode_detect.rs:165-174`.

### Constat

La liste couvre MP1/2/3, AAC, Vorbis et Opus, mais pas nécessairement tous les codecs avec perte
que Symphonia peut décoder.

### Piste de correction

- Définir le périmètre supporté dans la documentation.
- Ajouter les codecs réellement décodés et validés.
- Pour un codec inconnu, garder l'analyse mais ne jamais conclure qu'il est lossless.

# 2. Justesse des mesures DSP

## P1 - LUFS potentiellement faux en multicanal

**Fichiers :** `src-tauri/src/decode.rs:18-25`,
`src-tauri/src/signal_analysis.rs:163-175`.

### Constat

`DecodedAudio` conserve le nombre de canaux mais pas leur layout. `ebur128` reçoit donc seulement
le nombre de canaux et applique sa carte par défaut. En 5.1 et 7.1, la position des canaux, le
LFE et les surrounds ont une importance dans BS.1770.

### Impact

Une énergie placée dans le LFE, le centre ou les canaux surround peut produire un LUFS différent
de la référence.

### Piste de correction

- Conserver `AudioSpec::channels()` ou une représentation sérialisable du layout.
- Mapper explicitement les positions Symphonia vers `ebur128::Channel`.
- Retourner un état « layout inconnu » lorsque le mapping n'est pas fiable.
- Ne pas mélanger les canaux avec un downmix générique avant LUFS.

### Tests attendus

- 3.0, 5.0, 5.1, 7.1.
- Signaux isolés dans le LFE, le centre et les surrounds.
- Comparaison avec libebur128 ou ffmpeg.

## P1 - Seuil de clipping fixe et adapté au 16 bits

**Fichier :** `src-tauri/src/signal_analysis.rs:14-26,79-86`.

### Constat

`CLIPPING_THRESHOLD` vaut toujours `1 - 1/32768`, quelle que soit la profondeur déclarée. Ce
seuil détecte correctement la dissymétrie 16 bits, mais il considère comme écrêtés des échantillons
24 bits valides situés dans les derniers codes sous le plein échelle.

### Impact

Le nombre de clipping peut être exagéré sur les fichiers 24 bits. En plus, un échantillon proche
du plein échelle indique un échantillon full-scale, pas forcément une waveform aplatie.

### Piste de correction

- Adapter le seuil à la profondeur connue.
- Pour un codec sans profondeur PCM fiable, utiliser un état de mesure différent.
- Séparer `full_scale_sample_count` de `clipping_count`.
- Pour parler de clipping avéré, rechercher des répétitions, des plateaux ou des suites de
  samples au plafond.
- Modifier les textes qui disent que la forme d'onde est nécessairement aplatie.

### Tests attendus

- Rampes 16 bits et 24 bits juste sous le plein échelle.
- Square wave valide.
- Valeurs proches du plein échelle sans écrêtage.
- Signaux flottants et codecs sans profondeur déclarée.

## P1 - True peak non homogène à 192 kHz

**Fichiers :** `src-tauri/src/signal_analysis.rs:148-197`,
`src-tauri/Cargo.toml:36-39`.

### Constat

La documentation locale d'`ebur128` confirme : 4x sous 96 kHz, 2x entre 96 et 192 kHz, aucun
sur-échantillonnage à 192 kHz. Le code présente pourtant toujours le résultat comme un true peak
sur-échantillonné.

`true_peak(ch).unwrap_or(0.0)` transforme aussi une erreur de bibliothèque en `-120 dBTP`.

### Piste de correction

- Ajouter `oversampling_factor` au résultat.
- Dire « peak échantillonné » à 192 kHz si aucun calcul intersample n'est effectué.
- Vérifier si la feature `precision-true-peak` est acceptable après benchmark.
- Propager l'erreur au lieu de la convertir en zéro.
- Comparer des signaux inter-échantillons connus à 44.1, 96 et 192 kHz avec une référence.

## P1 - Détection de bit depth influencée par les silences

**Fichier :** `src-tauri/src/bit_depth.rs:85-108`.

### Constat

Les zéros sont alignés sur toutes les grilles. Avec un seuil de 99,9 %, un fichier réellement
24 bits contenant un très long silence peut être signalé comme 16 bits ou moins.

### Piste de correction

- Calculer l'alignement sur les échantillons actifs uniquement, avec un seuil documenté.
- Retourner le taux d'échantillons actifs et la couverture de l'observation.
- Refuser le résultat sur un fichier entièrement silencieux ou presque.
- Garder le faux négatif dû au dither comme limite assumée et l'afficher.

### Tests attendus

- 24 bits avec 99,9 % de silence.
- Une impulsion 24 bits non alignée sur 16 bits.
- Silence uniquement.
- 16 vers 24 bits avec dither.

## P2 - Plancher -120 dB qui masque la nature de la mesure

**Fichier :** `src-tauri/src/signal_analysis.rs:10-33`.

### Constat

Les valeurs nulles, très faibles et sous -120 dBFS sont toutes affichées à -120 dB. Cela touche
le peak, le RMS, le true peak et le crest factor.

### Piste de correction

- Retourner `null` pour une valeur non mesurable.
- Conserver éventuellement une valeur de plancher séparée pour l'affichage.
- Ajouter `is_floor_value` ou `measurement_status` dans les métriques.
- Afficher `n/d` pour un crest factor ou un peak qui n'a pas de sens sur le silence.

## P2 - LRA des fichiers courts possiblement affichée comme zéro valide

**Fichier :** `src-tauri/src/signal_analysis.rs:184-190`.

### Constat

`loudness_range()` peut retourner une valeur finie de `0.0` lorsque l'historique short-term est
vide ou trop court. Le code la sérialise alors comme une vraie LRA.

### Piste de correction

- Définir une durée minimale et un nombre minimal de blocs conforme à EBU Tech 3342.
- Retourner `None` en dessous de cette quantité.
- Ajouter une note « durée insuffisante » plutôt que simplement `n/d`.

### Tests attendus

Signaux de 1 s, 2,9 s, 5 s et 10 s, silence et deux niveaux distincts sur une durée courte.

## P2 - `effectively_mono` est actuellement inatteignable

**Fichier :** `src-tauri/src/stereo.rs:94-101,134-144`.

### Constat

`energy_ratio_db` plafonne la valeur à `-60 dB`, tandis que `effectively_mono` teste
`side_to_mid_db < -60 dB`. La condition ne peut donc jamais être vraie.

### Piste de correction

- Garder la valeur mesurée séparée de la valeur d'affichage.
- Tester `<= SIDE_NEGLIGIBLE_DB` ou utiliser un seuil strictement supérieur au plancher.
- Ajouter un test avec deux canaux presque identiques mais pas bit-identiques.

## P2 - Longueurs de canaux incohérentes

**Fichiers :** `src-tauri/src/decode.rs:112-126`, `src-tauri/src/metadata.rs:40-64`,
`src-tauri/src/spectral.rs:190-204`, `src-tauri/src/stereo.rs:72-78`.

### Constat

La durée et le nombre d'échantillons prennent le premier canal. Le spectre et la stéréo prennent
le plus court. En cas de canal tronqué, différentes sections du rapport parlent donc de durées
différentes.

### Piste de correction

- Imposer des longueurs de canaux identiques pendant le décodage.
- Ou exposer `min_sample_count`, `max_sample_count` et un état incomplet.
- Utiliser une longueur commune uniquement après avoir marqué le rapport comme dégradé.

## P2 - Spectre basé sur des hypothèses fortes sur le contenu

**Fichier :** `src-tauri/src/spectral.rs:34-45,427-475,496-518`.

### Constat

Le cutoff peak-relative est utile sur du contenu broadband, mais la musique réelle a souvent son
pic dans les médiums. L'occupation de bande, la moyenne de puissance et la moyenne par fichier
restent sensibles au genre, aux transitoires, aux passages calmes et à la largeur réelle du
contenu.

### Piste de correction

- Conserver des quantiles temporels plutôt qu'une seule moyenne globale.
- Mesurer séparément l'occupation broadband et la pente.
- Ajouter un score de qualité du matériau observé : énergie HF suffisante, nombre de fenêtres
  actives, largeur de bande utile.
- Ne pas faire monter la confiance quand la qualité du matériau est faible.

# 3. Mémoire, performance et pipeline

## P1 - Pic mémoire structurel trop élevé

**Fichiers :** `src-tauri/src/decode.rs:18-25`, `src-tauri/src/analysis.rs:73-162`,
`src-tauri/src/spectral.rs:206-212`, `.claude/CONTEXT.md:189-205`.

### Constat

Le pipeline conserve les buffers de tous les canaux et le buffer FFT complet. Le contexte du
projet documente environ 845 Mo de RSS pour 8 minutes en 96 kHz stéréo, après plusieurs
optimisations.

### Impact

Un fichier long en 192 kHz peut atteindre plusieurs gigaoctets et être tué par le système avant
la fin de l'analyse.

### Piste de correction

- Décoder et analyser par blocs.
- Faire les réductions RMS, peak, clipping, LUFS et DR pendant le décodage.
- Conserver uniquement les agrégats spectraux nécessaires au verdict.
- Générer le spectrogramme downsamplé au fil de l'eau au lieu de garder toutes les frames FFT.
- Garder un petit échantillon de fenêtres pour MDCT.
- Ajouter une limite ou un avertissement de ressources pour les fichiers extrêmes.

## P1 - Pas de progression ni d'annulation

**Fichier :** `src-tauri/src/commands.rs:11-24`.

### Constat

`spawn_blocking` évite de bloquer le thread événementiel, mais l'analyse reste sans progression
et sans annulation. Le contexte justifie l'absence de progression pour les mesures release
actuelles, mais le pic mémoire et la grille MDCT rendent cette hypothèse fragile.

### Piste de correction

- Donner un `analysis_id` à chaque analyse.
- Emettre des événements par étape et par bloc traité.
- Ajouter un token d'annulation partagé.
- Afficher la phase courante : décodage, sonie, spectre, grille, finalisation.

## P1 - Nouveau protocole média qui relit tout le fichier sans Range

**Fichier :** `src-tauri/src/media_protocol.rs:135-164`.

### Constat

Les requêtes avec `Range` sont limitées à 1 MiB, mais une requête sans `Range`, y compris une
requête `HEAD`, passe dans la branche qui fait `read_to_end` du fichier entier.

### Impact

Le nouveau protocole peut annuler le bénéfice du streaming et charger plusieurs centaines de Mo
ou plusieurs Go pour une seule requête média.

### Piste de correction

- Répondre correctement à `HEAD` sans corps.
- Pour un `GET` sans Range, servir un premier bloc borné ou gérer un flux adapté à l'API Tauri.
- Ne jamais faire `Vec::with_capacity(len as usize)` sans garde de taille.
- Tester les requêtes réellement émises par WebKit sur FLAC longs.

## P1 - Range inversé pouvant provoquer un underflow

**Fichier :** `src-tauri/src/media_protocol.rs:135-146,171-181`.

### Constat

`bytes=100-0` est accepté par `parse_range`. La branche suivante calcule `end - start + 1`, ce
qui provoque un underflow en debug et une valeur énorme en release.

### Piste de correction

- Rejeter les ranges où `end < start` avec `416 Range Not Satisfiable`.
- Implémenter les ranges suffixes `bytes=-500` ou les rejeter explicitement avec `416`, pas en
  les transformant en réponse complète.
- Ajouter des tests aux bornes `0-0`, `len-1-len-1`, `len-len+1`, inversés et suffixes.

## P2 - MIME du protocole basé sur l'extension, pas sur le média analysé

**Fichier :** `src-tauri/src/media_protocol.rs:76-94`.

### Constat

Un fichier dont l'extension est incorrecte reçoit un MIME incorrect. Le probe Symphonia peut
pourtant réussir grâce au contenu réel.

### Impact

L'analyse peut réussir mais la lecture WebKit échouer ou interpréter la ressource avec le mauvais
type.

### Piste de correction

- Passer le MIME déterminé au moment de l'autorisation, après l'analyse.
- Ou déterminer le MIME via les premiers octets, avec une table validée pour les formats supportés.
- Ajouter `.opus`/`.oga` si ces formats sont réellement acceptés par le backend.

## P2 - Scope média sensible à une course symlink

**Fichier :** `src-tauri/src/media_protocol.rs:54-73,112-125`.

### Constat

Le chemin est canonicalisé pour le contrôle, puis ouvert séparément. Un changement de symlink
entre ces deux opérations peut modifier la cible ouverte.

### Piste de correction

- Ouvrir le fichier puis vérifier la cible de manière atomique autant que les APIs de la plateforme
  le permettent.
- Ou stocker une autorisation par chemin canonique et refuser les chemins qui changent de cible.
- Ajouter un test de remplacement de symlink sur les plateformes supportées.

# 4. Interface et expérience utilisateur

## P1 - Courses entre analyses concurrentes

**Fichier :** `src/routes/+page.svelte:122-140`.

### Constat

`analyze()` ne possède pas d'identifiant de requête. Deux drops ou sélections rapides peuvent
laisser l'analyse ancienne remplacer la nouvelle. `loading` peut aussi repasser à `false` trop
tôt.

### Piste de correction

- Ajouter un compteur ou token d'analyse.
- Capturer le token dans chaque promesse et ignorer les résultats obsolètes.
- Séparer l'état de l'analyse principale de celui de la comparaison.
- Désactiver ou annuler explicitement une analyse précédente.

## P1 - Autorisation de lecture couplée à l'analyse

**Fichier :** `src/routes/+page.svelte:131-135`.

### Constat

`Promise.all([analyzeFile(path), authorizePlayback(path)])` masque le rapport si l'autorisation
du lecteur échoue.

### Piste de correction

- Afficher le rapport dès que `analyzeFile()` réussit.
- Stocker `playbackError` séparément.
- Afficher un lecteur désactivé avec une explication, sans perdre le résultat technique.

## P1 - Erreurs audio silencieuses

**Fichier :** `src/routes/+page.svelte:173-176,755-784`.

### Constat

La promesse de `audioEl.play()` n'est pas capturée. Les événements média mettent actuellement
à jour un diagnostic temporaire, mais aucun message utilisateur n'explique qu'un format n'est
pas lisible par WebKit.

### Piste de correction

- Ajouter `try/catch` autour de `play()`.
- Ajouter un état `playbackError` et une traduction.
- Distinguer « analyse réussie » de « lecture indisponible ».
- Ajouter un état `canplay` avant d'activer le bouton de lecture.

## P2 - Ancienne lecture pas explicitement arrêtée

**Fichier :** `src/routes/+page.svelte:122-140`.

### Constat

Lors d'un nouveau fichier, `audioSrc` est mis à `null`, mais l'élément média existant n'est pas
explicitement mis en pause et remis à zéro.

### Piste de correction

Avant de remplacer la source : appeler `pause()`, mettre `currentTime` à zéro et appeler `load()`
si nécessaire. Réinitialiser aussi `audioEl` et `isPlaying` après le démontage.

## P2 - Erreurs de comparaison invisibles et erreurs principales potentiellement dupliquées

**Fichiers :** `src/routes/+page.svelte:143-159,364-380`.

### Constat

Une erreur de comparaison est mise dans `error`, mais aucun bloc n'est rendu lorsque `result`
existe. Dans certains chemins, les messages d'erreur principaux peuvent être affichés à deux
endroits.

### Piste de correction

- Ajouter `compareError`.
- Afficher l'erreur sous le bouton ou près de la vue de comparaison.
- Centraliser le rendu de l'erreur principale.

## P2 - Export sans gestion d'exception ni confirmation

**Fichier :** `src/routes/+page.svelte:202-210`.

### Constat

Les erreurs de `open()`, `save()` et `exportReport()` remontent sans feedback dédié. L'utilisateur
n'a pas de confirmation quand le rapport est écrit.

### Piste de correction

- Encapsuler chaque dialogue et l'IPC dans un `try/catch`.
- Ajouter un message de succès temporaire.
- Afficher le chemin choisi si cela reste compatible avec l'UX.

## P2 - Mute impossible à restaurer après volume zéro

**Fichier :** `src/routes/+page.svelte:188-190,734-745`.

### Constat

Le bouton annonce « rétablir le son » lorsque `volume === 0`, mais `toggleMute()` ne restaure
aucun ancien volume non nul.

### Piste de correction

Mémoriser `lastAudibleVolume` avant de passer à zéro et le restaurer au dé-muet.

## P2 - Durée affichable sous la forme `0:60`

**Fichiers :** `src/routes/+page.svelte:213`, `src/lib/components/Comparison.svelte:31-32`,
`src-tauri/cli/src/main.rs:79-82`, `src/lib/components/Spectrogram.svelte:104-108`.

### Piste de correction

Utiliser `Math.floor` pour les secondes ou normaliser le résultat de l'arrondi avant de construire
la chaîne minutes/secondes.

## P2 - Vue comparaison trop pauvre en preuves

**Fichier :** `src/lib/components/Comparison.svelte:235-249`.

### Constat

La comparaison montre les verdicts et les scores, mais pas les indices explicatifs. Le lecteur
reste lié au fichier principal sans annoncer clairement quel fichier est lu.

### Piste de correction

- Ajouter les principaux indicateurs sous chaque verdict.
- Afficher le nom du fichier actuellement joué.
- Ajouter une indication claire « lecture : A » ou « lecture : B » si le lecteur est étendu.

## P2 - Comparaison : informations de qualité codées uniquement par couleur

**Fichier :** `src/lib/components/Comparison.svelte:266-269,445-452`.

### Constat

Le côté « meilleur » change principalement de couleur. Cela ne suffit pas pour le daltonisme,
le contraste faible ou un lecteur d'écran.

### Piste de correction

Ajouter un texte, une icône et un libellé ARIA du type « marge supérieure pour cette mesure ».

## P2 - Textes trop catégoriques sur des heuristiques

**Fichiers :** `src/lib/i18n.svelte.ts:299-314,332-343`,
`src/lib/components/MdctGrid.svelte:109-123`.

### Exemples

- « le fichier a été rembourré » alors que la grille ne couvre qu'environ 99,9 % des samples.
- « la waveform a été aplatie » alors que le code compte des samples proches du plein échelle.
- « la fréquence est gonflée » pour un résultat de sample-rate heuristique.
- « l'audio sans perte n'a aucun alignement de ce genre » pour une détection statistique.

### Piste de correction

Employer « compatible avec », « probablement », « indice d'un padding » et « peut indiquer ».
Afficher le taux observé, le périmètre de la mesure et la limite connue.

## P2 - Repères présentés comme des vérités universelles

**Fichiers :** `src/lib/i18n.svelte.ts:332-343,520-535`,
`src/lib/components/Comparison.svelte:109-183`.

### Constat

- `-14 LUFS` est présenté comme la cible streaming universelle.
- `> 0 dBTP` est présenté comme une certitude de clipping ultérieur.
- La DR est étiquetée « bonne » ou « fortement compressée » alors que le contexte artistique
  compte.
- Une bande passante plus large est marquée comme meilleure dans la comparaison.

### Piste de correction

Qualifier ces valeurs de repères ou conventions. Dire « peut clipper après conversion » et
présenter la DR comme une mesure conventionnelle, pas comme une note de qualité.

## P2 - Spectrogramme avec rôle clavier incohérent

**Fichier :** `src/lib/components/Spectrogram.svelte:37-59,147-157`.

### Constat

Le conteneur possède toujours `role="button"` et `tabindex="0"`, même lorsque `onSeek` est
absent en comparaison. Les touches Entrée/Espace n'ont pas le comportement attendu d'un bouton.

### Piste de correction

- Utiliser `role="slider"` avec `aria-valuenow`, `aria-valuemin`, `aria-valuemax` et les touches
  gauche/droite/Home/End.
- Ou ne rendre le conteneur focusable et contrôlable que si `onSeek` existe.
- Ajouter une vraie gestion Entrée/Espace si le rôle reste `button`.

## P2 - Accessibilité des mètres et niveaux par bande

**Fichiers :** `src/lib/components/Meter.svelte:61-81`,
`src/lib/components/BandLevels.svelte:47-69`.

### Constat

Les mètres n'ont pas de `aria-label` ou `aria-labelledby`, ni de `aria-valuetext` avec unité.
Les bandes reposent surtout sur `title` et sur le survol.

### Piste de correction

- Passer un label au composant `Meter`.
- Ajouter `aria-valuetext`, par exemple `-14 LUFS` ou `-1 dBTP`.
- Fournir une table ou une liste visuellement masquée pour les bandes.

## P2 - Responsive incomplet

**Fichiers :** `src/routes/+page.svelte:881-919,1431-1461`,
`src/lib/components/Comparison.svelte:252-274`.

### Constat

La barre supérieure contient plusieurs actions sans stratégie de repli à la largeur minimale.
Les tables n'ont pas de viewport horizontal. La table de canaux peut déborder à petite largeur.

### Piste de correction

- Autoriser le retour à la ligne ou regrouper les actions secondaires.
- Entourer les tableaux d'un conteneur avec `overflow-x: auto`.
- Tester la fenêtre à 560 px, sur petit écran et avec les textes français les plus longs.

## P2 - Contraste et compatibilité WebKit

**Fichiers :** `src/routes/+page.svelte:804-834,1274-1315`,
`src/lib/components/Meter.svelte:112-117`,
`src-tauri/tauri.conf.json:44-45`.

### Constat

Les couleurs `--ink-low` sont très faibles, notamment sur les petits labels. Plusieurs éléments
reposent sur `color-mix()`, alors que le minimum macOS annoncé est 10.15.

### Piste de correction

- Vérifier les contrastes WCAG en clair et en sombre.
- Utiliser des couleurs opaques dédiées aux textes.
- Ajouter des fallbacks avant `color-mix()` et une règle `@supports`.
- Tester la version WebKit réellement ciblée.

## P3 - Détails UI supplémentaires

- `src/lib/components/MdctGrid.svelte:31-57` : absence de `ResizeObserver`, le canvas ne se
  redessine pas après redimensionnement.
- `src/lib/components/Spectrogram.svelte:184-197,323-349` : pastilles de palette petites et
  navigation radio non roving.
- `src/lib/components/MdctGrid.svelte:109-120` : `toFixed()` n'est pas localisé en français.
- `src/routes/+page.svelte:73-75,116-119` : accès au `localStorage` du thème non protégé,
  contrairement au volume et à la langue.
- `src/routes/+page.svelte:147-167` : `.opus` n'est pas proposé dans le sélecteur alors que le
  backend reconnaît Opus.
- `src/routes/+page.svelte:120-121,349-365` : les messages backend restent en anglais en
  français.
- `src/routes/+page.svelte:46-60,750-753,1517-1533` : les diagnostics temporaires doivent être
  supprimés avant une livraison.

# 5. Contrat IPC et sécurité

## P2 - Types TypeScript sans validation runtime

**Fichiers :** `src/lib/api.ts:119-241`, `src/lib/i18n.svelte.ts:264-297`.

### Constat

Les types reflètent le backend, mais `invoke()` ne valide pas les données reçues. Une valeur
hors plage, un base64 invalide ou un nouveau code d'indicateur peut atteindre l'interface.

### Piste de correction

- Ajouter une validation à la frontière IPC, avec une bibliothèque légère ou des validateurs
  ciblés.
- Vérifier les plages numériques, tailles de tableaux, base64 et enums.
- Fournir un fallback pour un indicateur inconnu.
- Ajouter un golden JSON pour verrouiller le contrat.

## P2 - Pas de version ou qualité dans le contrat d'analyse

**Fichiers :** `src-tauri/src/analysis.rs:24-45`, `src/lib/api.ts:216-228`.

### Piste de correction

Ajouter des champs explicites :

- `analysis_version`.
- `decode_status`.
- `measurement_quality`.
- `spectral_bandwidth_status`.
- `confidence_kind`.

Cela évite de faire passer une valeur de repli, une absence de mesure et un résultat fiable pour
le même type de donnée.

## P2 - Protocole média : couverture de tests insuffisante

**Fichier :** `src-tauri/src/media_protocol.rs:214-264`.

Les tests actuels couvrent MIME, parsing simple, percent decoding et scope minimal, mais pas :

- `HEAD`.
- GET sans Range sur gros fichier.
- Range inversé.
- Range dépassant la fin.
- Range suffixe.
- chemins Windows.
- chemins absolus macOS/Linux.
- fichier remplacé après autorisation.
- `Content-Length` et `Content-Range` réellement servis.

# 6. Tests et corpus

## P1 - Corpus trop limité pour régler de nouveaux seuils

**Fichiers :** `src-tauri/tests/fixtures/corpus/README.md:3-8,67-76`,
`src-tauri/tests/fixtures/generate_corpus.sh`.

### Limites actuelles

- Corpus principalement synthétique.
- Beaucoup de bruit stationnaire.
- Plusieurs fixtures dual-mono.
- Un seul encodeur AAC principal.
- Peu de vraies signatures de mastering et de genres musicaux.
- Pas assez de short blocks, layouts multicanaux ou contenus ré-échantillonnés complexes.

### Piste de correction

Ajouter au minimum :

- MP3 LAME V0, V2, V5, CBR et VBR.
- AAC LC avec plusieurs encodeurs et bitrates.
- Vorbis et Opus pour vérifier les formats déclarés avec perte.
- Musique tonale, piano, voix, orchestral, percussions et contenu naturellement sombre.
- Silences, fades, transitoires et passages avec forte dynamique.
- 44.1, 48, 96 et 192 kHz.
- Mono, stéréo réelle, dual-mono, anti-phase et 5.1.
- Padding de bit depth avec et sans dither.
- Fichiers tronqués et checksum invalide.

Les seuils doivent être réglés sur une partie du corpus et évalués sur une partie tenue à l'écart.

## P1 - Tests manquants par fonctionnalité

### Décodage et intégrité

- Erreurs de paquets.
- `ResetRequired`.
- FLAC MD5 valide, nul et invalide.
- Changement de longueur entre canaux.

### Signal

- LUFS de référence.
- LRA de référence.
- True peak intersample connu.
- Signal silencieux.
- Valeurs sous -120 dBFS.
- Clipping 16, 24 et 32 bits.

### DR14

- Plusieurs blocs de niveaux différents.
- Second plus haut pic.
- Top 20 %.
- Transitoire isolé.
- Comparaison avec une implémentation numpy de référence.

### Spectre

- Notch rejeté suivi d'un candidat valide.
- Tonalité pure.
- Contenu bass-only.
- Master naturellement sombre.
- Coupure localisée dans le temps.
- Contenu hi-res naturellement limité.

### MDCT

- Premier canal silencieux.
- Stéréo réelle.
- Short blocks.
- KBD.
- Fichier trop court.
- Fichier silencieux.

### Stereo

- `effectively_mono`.
- Corrélation négative.
- Mid nul et side non nul.
- Bandes hors Nyquist.

### Contrat

- Sérialisation JSON complète.
- Compatibilité TypeScript.
- Tous les verdicts, y compris `declared_lossy`.
- Codes d'indicateurs exhaustifs.

### Interface

- Deux analyses simultanées.
- Erreur de lecture après analyse réussie.
- Erreur d'export.
- Comparaison échouée.
- Mute après volume zéro.
- Navigation clavier du spectrogramme.
- Petite largeur de fenêtre.

# 7. Documentation, CI et maintenance

## P2 - Documentation obsolète

### Incohérences repérées

- `README.md:53-57` parle encore d'un verdict à trois états au lieu de quatre.
- `README.md:113-120` indique encore que AAC 256 est totalement non détecté alors que la grille
  MDCT le détecte sur le corpus.
- `src-tauri/tests/fixtures/corpus/README.md:88` indique encore que l'AAC 128 dynamique est
  indéterminé à 30 %, alors que la sortie actuelle du test le classe probablement transcodé à
  90 % grâce à la grille MDCT.
- `src-tauri/src/transcode_detect.rs:6` parle de trois états.
- `.claude/CLAUDE.md:31,174-178` ne reflète pas complètement la couverture AAC actuelle.
- `CHANGELOG.md:254-258` parle d'une release publiée en brouillon, tandis que
  `.github/workflows/release.yml:137` utilise `releaseDraft: false`.
- Le commentaire de `spectral.rs:5-7` dit que le scoring V0.3 n'est pas encore écrit, alors qu'il
  existe.
- `.claude/CONTEXT.md` décrit encore principalement `asset://`, alors que le worktree contient
  un protocole personnalisé.

### Piste de correction

Faire une passe documentaire après chaque changement du contrat ou du scoring. Les limites
connues, surtout LAME V0 et le périmètre MDCT, doivent être visibles dans le README et dans l'UI.

## P2 - Workflow de release insuffisamment protégé

**Fichier :** `.github/workflows/release.yml:20-138`.

### Constat

Le workflow de release vérifie les versions mais ne dépend pas explicitement des tests complets,
du clippy ou du build frontend avant publication. Il publie directement avec `releaseDraft: false`.

Les actions externes utilisent des tags flottants (`@v0`, `@stable`, `@v4`).

### Piste de correction

- Faire dépendre la release d'un job de validation identique à la CI PR.
- Publier en draft tant qu'une vérification manuelle des artefacts n'est pas faite.
- Pinner les actions critiques sur des SHA validés.
- Documenter le risque des builds non signés séparément de la qualité de l'analyse.

## P3 - `cargo fmt --check` en échec

Le formatage Rust existant n'est pas conforme à `rustfmt`. Ce n'est pas un bug fonctionnel,
mais ajouter `cargo fmt --all -- --check` à la CI évitera que la divergence augmente. La commande
doit être lancée après avoir décidé si le reformatage global est acceptable dans une PR séparée.

## P3 - Dépendance opener probablement inutilisée

`tauri-plugin-opener` est présent dans `Cargo.toml`, `package.json`, les capabilities et initialisé
dans `src-tauri/src/lib.rs:20`, mais aucune utilisation frontend ou backend n'a été trouvée.

### Piste de correction

Supprimer le plugin et sa permission si aucun besoin réel ne subsiste. Sinon ajouter la fonction
qui justifie sa présence.

## P3 - Exclusion Windows Defender très large en CI

**Fichiers :** `.github/workflows/build.yml:52-61`, `.github/workflows/release.yml:74-85`.

L'exclusion de tout le workspace et de plusieurs processus accélère les runners éphémères, mais
réduit la protection pendant le build. Ce n'est pas une vulnérabilité de l'application livrée,
mais c'est un compromis à documenter et à limiter aux runners de confiance.

# 8. Plan d'amélioration recommandé

## Phase 1 - Sécuriser le verdict

1. Ajouter le statut de décodage complet et neutraliser le verdict sur audio incomplet.
2. Retourner `Indeterminate` lorsque le système n'a qu'une absence d'indice.
3. Séparer bande passante mesurée, edge d'encodeur et Nyquist.
4. Corriger la sélection des candidats spectraux.
5. Ajouter une qualité de mesure et une confiance non présentée comme probabilité.
6. Mettre à jour le corpus et documenter le taux de ratés connu.

## Phase 2 - Corriger les mesures

1. Préserver et mapper les layouts multicanaux pour LUFS.
2. Corriger LRA court et erreurs true peak.
3. Rendre le clipping dépendant de la profondeur et distinguer full-scale/clipping.
4. Exclure ou pondérer les silences dans la détection de bit depth.
5. Corriger `effectively_mono`.
6. Ajouter les références numériques LUFS/LRA/true peak/DR14.

## Phase 3 - Stabiliser le pipeline

1. Corriger le mismatch `protocol-asset` du worktree actuel.
2. Tester et sécuriser le protocole média personnalisé.
3. Ajouter requête tokenisée, annulation et progression.
4. Commencer le refactor streaming pour les métriques et le spectre.

## Phase 4 - Corriger l'interface

1. Séparer erreurs d'analyse, de lecture, de comparaison et d'export.
2. Ajouter les indices dans la comparaison.
3. Revoir tous les textes qui transforment une heuristique en certitude.
4. Corriger clavier, ARIA, contraste et responsive.
5. Supprimer les diagnostics temporaires.

## Phase 5 - Réconcilier le dépôt

1. Mettre à jour README, changelog, contexte et commentaires obsolètes.
2. Ajouter les tests de contrat JSON et du protocole média.
3. Faire passer `cargo fmt --check`.
4. Faire dépendre les releases de la CI complète.

# 9. Conclusion

Le code n'est pas dans un état inquiétant côté structure : les modules sont séparés, les choix
DSP sont documentés et les tests existants ont déjà éliminé plusieurs faux positifs importants.

La faiblesse actuelle est sémantique et méthodologique : plusieurs résultats donnent plus de
certitude que la mesure ne le permet. Le chantier prioritaire est donc de rendre le rapport
honnête et traçable avant de chercher à attraper davantage de codecs.

Le point le plus urgent reste LAME V0 : tant que l'absence de coupure entraîne
`ProbablyAuthentic`, le produit peut cautionner un faux lossless connu. Le meilleur premier
changement est de retourner `Indeterminate` dans ce cas, puis de construire de nouvelles preuves
positives sur un corpus plus varié plutôt que d'abaisser encore les seuils spectraux.
