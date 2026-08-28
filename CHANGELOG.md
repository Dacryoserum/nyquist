# Changelog

All notable changes to this project are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer once
releases start shipping.

## [Unreleased]

## [0.5.0] - 2026-08-28

Une version de correction. Elle ne cache plus rien : le verdict refuse de cautionner ce
qu'il ne peut pas voir, la lecture ne dépend plus du navigateur, et les mesures qui
n'étaient pas mesurées le disent.

### Modifié

- **La lecture ne passe plus par le navigateur.** L'audio est joué nativement, à partir des
  échantillons que l'analyse vient de décoder. L'élément `<audio>` du webview se forgeait sa
  propre idée de la durée du fichier, en parsant le fichier de son côté, pendant que le
  curseur, l'axe du spectrogramme et le rapport utilisaient celle mesurée par le décodeur.
  Trois symptômes découlaient de cette seule divergence : un clic sur le spectrogramme
  tombait à côté, le compteur dérivait, et les morceaux longs s'arrêtaient avant la fin.
  Deux transports successifs (`asset://` puis un serveur HTTP local) avaient été essayés
  avant de comprendre que le problème n'était pas le transport : deux horloges ne se
  synchronisent pas en améliorant le coursier entre elles. Il n'y en a plus qu'une —
  un index d'échantillon dans la piste décodée — et chaque chiffre affiché en découle.
  Le seek est désormais exact à l'échantillon près.

- **Un fichier sans indice de transcodage n'est plus déclaré « probablement authentique ».**
  Ne rien trouver n'est pas la même chose que trouver que tout va bien. Un encodage MP3
  transparent (LAME V0) ne coupe pas les aigus du tout, et l'outil rendait pourtant un verdict
  rassurant sur les deux fichiers de ce type de son propre corpus de test — le pire résultat
  possible pour un outil dont le rôle est de repérer un mensonge. Ces fichiers ressortent
  maintenant en « indéterminé ». « Probablement authentique » demande désormais une preuve
  positive : du contenu réel dans le haut d'une bande hi-res, qu'aucun encodage à la fréquence
  du CD n'aurait pu y mettre. Sur le corpus : aucun fichier réellement transcodé n'est plus
  cautionné, et toujours zéro faux positif.
- **La confiance s'affiche comme une force d'indices — faible, modérée, forte — et non plus en
  pourcentage.** Les nombres sous-jacents sont des poids réglés sur vingt fichiers de test, pas
  des probabilités calibrées ; « 90 % » laissait croire à une précision qui n'existe pas. Le
  nombre brut reste dans le rapport JSON exporté.
- **Un décodage incomplet suspend le verdict.** Si des paquets ont été sautés, ou si le flux
  s'est arrêté en cours de route, les mesures ne décrivent qu'un fragment : elles restent
  affichées, mais l'outil ne rend plus de jugement sur le fichier entier.
- **La bande passante non mesurable s'affiche comme telle.** Quand le balayage ne trouve aucun
  point où le contenu s'arrête, l'app le dit, au lieu de reporter la fréquence de Nyquist comme
  si elle avait été mesurée — ce qui présentait un morceau uniquement grave comme occupant
  100 % de sa bande.
- **Les échantillons au plein échelle et les passages réellement écrêtés sont comptés
  séparément.** Un échantillon isolé qui touche le plafond est un transitoire fort ; ce sont les
  suites d'échantillons plaqués qui signalent une forme d'onde aplatie. Le seuil suit aussi la
  profondeur déclarée du fichier : en 24 bits, l'ancien seuil fixe comptait les 256 valeurs les
  plus hautes comme écrêtées.
- **Les formulations catégoriques ont été nuancées.** « Le fichier a été rembourré » devient
  « c'est compatible avec un rembourrage » ; les repères -14 LUFS et DR 12 sont présentés comme
  des conventions, pas comme des notes.

### Ajouté

- **Le rapport exporté indique la version du pipeline qui l'a produit.** Les seuils et la
  logique de verdict évoluent d'une version à l'autre ; sans cela un rapport relu plus tard
  n'est pas interprétable.
- **Lecture des fichiers Opus** (`.opus`, `.oga`), qui manquaient au sélecteur alors que le
  moteur d'analyse les décodait déjà.

### Corrigé

- **La plage de sonie (LRA) n'est plus affichée sur des fichiers trop courts.** En dessous de
  dix secondes, la fenêtre glissante de trois secondes de l'EBU Tech 3342 ne voit pas assez de
  matière distincte, et la bibliothèque renvoyait un 0.0 qui s'affichait comme une mesure.
- **La détection de « faux hi-res » ne se laisse plus décider par le silence.** Le silence
  numérique tombe sur toutes les grilles de quantification à la fois : un vrai master 24 bits
  contenant un long passage muet était signalé comme du 16 bits rembourré. La mesure ne porte
  plus que sur les échantillons actifs, et se tait quand il n'y en a pas assez.
- **L'indicateur « mono de fait » ne se déclenchait jamais.** Le test comparait le rapport
  side/mid à la valeur plancher à laquelle l'affichage venait de l'écrêter, donc il était faux
  pour tous les fichiers.
- **Une erreur de la bibliothèque de mesure ne se transforme plus en -120 dBTP.** La crête
  réelle est aussi étiquetée correctement à 192 kHz et au-delà, où aucun sur-échantillonnage
  n'est appliqué : c'est un pic échantillonné, pas une crête inter-échantillon.
- **Deux analyses lancées coup sur coup ne mélangent plus leurs résultats.** Chaque analyse
  porte un jeton, et une réponse qui arrive après qu'une autre a démarré est ignorée au lieu
  d'écrire par-dessus le rapport affiché.
- **Une lecture impossible n'efface plus un rapport valide.** Analyse et lecture sont deux
  résultats distincts : le rapport s'affiche dès que l'analyse aboutit, et l'échec de lecture
  est signalé à part, près du lecteur.
- **Les échecs de lecture sont enfin visibles.** Un refus de `play()`, un format que le lecteur
  ne sait pas décoder, une lecture qui cale : chacun affiche maintenant un message au lieu
  d'échouer en silence.
- **L'export JSON signale les erreurs et confirme le succès**, au lieu de laisser croire que le
  fichier a été écrit.
- **Le curseur de progression ne saute plus pendant une navigation au clavier.** Le verrou
  n'était armé que par le pointeur.
- **Le bouton muet restaure le volume précédent** quand le curseur a été descendu à zéro.
- **Une durée ne s'affiche plus « 0:60 ».**
- **Une erreur de comparaison s'affiche près de la comparaison**, au lieu de prendre la place du
  message d'erreur principal.
- **Des canaux de longueurs différentes sont signalés.** Ils ne l'étaient nulle part, alors que
  chaque section du rapport en tirait une longueur différente : la durée lisait le premier
  canal, le spectre et la stéréo le plus court. Le fichier est désormais marqué comme
  incomplet, et le verdict suspendu avec.
- **La grille MDCT s'analyse sur le canal le plus énergétique**, pas sur le premier. Un fichier
  dont le premier canal est muet était balayé sur du silence et rendait un résultat propre
  qu'il n'avait jamais mesuré.
- **Une lecture indisponible dit pourquoi.** L'échec du chargement était avalé : l'interface
  affichait « indisponible » sans jamais nommer la cause.

### Sécurité

- **L'URL de lecture ne contient plus de chemin de fichier.** L'autorisation ouvre le fichier et
  l'URL désigne ce descripteur ouvert par un identifiant aléatoire. Il n'existe donc plus aucun
  nom à détourner entre le contrôle et la lecture : remplacer un lien symbolique après coup ne
  peut plus faire servir un autre fichier. Un fichier réécrit sous l'URL n'est pas servi non
  plus — l'URL est retirée, pour qu'un cache ne puisse jamais rendre un contenu périmé.
- **Le serveur de lecture démarre sous Windows.** Il tirait son jeton aléatoire de
  `/dev/urandom`, un chemin qui n'existe pas là-bas : l'application plantait au lancement avant
  d'afficher quoi que ce soit. Un échec de démarrage ne fait d'ailleurs plus planter l'app du
  tout — l'analyse fonctionne, seule la lecture est signalée indisponible.
- **Le serveur de lecture ne peut plus être saturé par un processus local.** Nombre de
  connexions simultanées plafonné, taille des requêtes bornée, délai d'écriture, et une
  requête malformée reçoit une réponse d'erreur au lieu de faire déborder un calcul : une plage
  d'octets inversée (`bytes=100-50`) provoquait un dépassement de capacité.

### Corrigé (lecture audio)

- **Un morceau long s'arrêtait avant la fin, sans erreur ni message.** Le webview concluait
  que le fichier se terminait là où le son s'était tu — il révisait même sa propre estimation
  de la durée. Trois choses s'additionnaient. Tauri annonçait les FLAC en `audio/x-flac`, une
  forme ancienne que WebKit ne reconnaît pas et qui le pousse à deviner la durée en reniflant
  le flux. Le protocole `asset://` comme un schéma d'URI maison plafonnaient la livraison vers
  32 Mio, ce qui coupait la dernière minute d'un fichier de 38 Mio. Enfin, les réponses
  interdisaient toute mise en cache : comme macOS demande à une page passée en arrière-plan
  d'être économe, WebKit jetait ce qu'il avait téléchargé et devait tout redemander, sans
  filet. L'audio passe maintenant par un petit serveur local, avec le type MIME que WebKit
  attend, des réponses qu'il a le droit de garder, et des connexions réutilisées d'une plage
  à la suivante plutôt qu'une trentaine de connexions par morceau. À noter : ce qui est
  mesuré, c'est que ce transport livre le fichier entier là où les deux autres ne le
  faisaient pas. Le mécanisme exact à l'intérieur de WebKit reste une hypothèse, pas une
  cause démontrée — voir `.claude/audits/INVESTIGATION-lecture-tronquee.md`.
- **Le lecteur se relève d'une fin prématurée.** L'analyse mesure la durée exacte du fichier,
  donc l'app peut reconnaître une fin qui arrive trop tôt : elle recharge alors la source et
  reprend là où le son s'était arrêté. Cinq fois au plus, et toujours vers l'avant, pour qu'un
  fichier qui se termine vraiment tôt ne tourne pas en boucle. C'est un contournement, pas une
  correction : si les reprises s'épuisent sans atteindre la fin, l'app le dit maintenant au
  lieu de laisser la lecture passer pour normale.
- **Le curseur de progression pouvait rester bloqué.** Relâcher le pointeur ailleurs que sur la
  barre ne déclenchait pas l'événement attendu, et le temps affiché restait figé pour le reste
  de la session.

### Sécurité

- **La lecture ne passe plus par le protocole `asset://` de Tauri, désormais désactivé.** Le
  serveur local n'écoute que sur `127.0.0.1`, chaque URL porte un jeton aléatoire tiré au
  lancement de l'app, et seuls les fichiers que l'app a explicitement autorisés sont servis :
  tout le reste est refusé, y compris des chemins parfaitement lisibles par l'utilisateur.

### Supprimé

- **Le serveur HTTP local de lecture** (679 lignes écrites à la main) et toute sa surface :
  plus de port ouvert, plus de jeton, plus d'en-têtes HTTP, plus de plages d'octets. Le
  webview ne reçoit plus aucun média (`media-src 'none'`).
- **La reprise automatique de lecture**, qui rechargeait la source et revenait là où le son
  s'était arrêté. Elle contournait un défaut qui ne peut plus se produire : la fin d'une
  piste est maintenant un index d'échantillon connu.
- **Le bandeau de diagnostic temporaire** qui restait affiché sous le lecteur.
- **La dépendance `tauri-plugin-opener`**, initialisée et déclarée dans les permissions sans
  qu'aucun code ne s'en serve.

## [0.4.0] - 2026-08-24

### Corrigé

- **Les boutons de la barre du haut avaient doublé de taille.** `.ghost` était défini dans le
  `<style>` scopé de la page, donc Svelte le compilait avec une spécificité qui battait le
  reset `button { font: inherit }` juste au-dessus. En le passant en global pour que les
  composants enfants puissent s'en servir, il est repassé sous ce reset et les boutons ont
  hérité de la police du body, à sa taille. Le reset est global lui aussi maintenant : la
  spécificité tranche, plus l'ordre des règles.
- Le bouton de fermeture de la comparaison n'avait littéralement aucun style, pour la même
  raison — c'était un `<button>` HTML brut.
- **Un MP3 n'est plus accusé de transcodage.** Le verdict de cet outil répond à une question
  précise : « ce fichier *sans perte* cache-t-il de l'audio avec perte ? » La poser sur un
  MP3 est une erreur de catégorie, et elle produisait une réponse absurde — un MP3 ordinaire
  sortait « probablement transcodé » à 80 %, et un fichier AAC à 95 % parce que la détection
  de grille MDCT y trouvait, correctement, la grille qui est *censée* s'y trouver. Rien n'est
  dissimulé dans ces cas-là. Un quatrième état, « format avec perte, annoncé », le dit sans
  pourcentage — ce n'est pas une inférence, le conteneur l'affirme. **Toutes les mesures
  restent affichées**, y compris le passe-bas et la grille de l'encodeur, qui renseignent sur
  ses réglages. Un FLAC cachant de l'AAC reste évidemment détecté (90 %).

### Modifié

- **Vue de comparaison complétée et restructurée.** Elle ne montrait qu'un sous-ensemble des
  mesures ; sonie, dynamique, détail spectral et image stéréo y sont désormais présents. Le
  tableau est groupé en cinq sections plutôt qu'une liste plate, les spectrogrammes sont côte
  à côte (et ne s'empilent que sous 700 px, bien en dessous des 1000 px par défaut de la
  fenêtre), et les grilles MDCT restent alignées côte à côte.
- **Niveaux par bande en profil de colonnes** plutôt qu'en huit lignes empilées. Ces niveaux
  sont une *courbe* — où l'énergie se situe et où elle s'arrête — et une liste demande au
  lecteur de la reconstituer mentalement. La valeur en dB est posée sur chaque bâton, au-
  dessus ou à l'intérieur selon la place disponible. Bénéfice secondaire : la carte « Détail
  spectral » cesse de dépasser d'une tête sa voisine.
- **Cartes appairées de même hauteur.** Elles s'étirent désormais l'une sur l'autre, et la
  note de bas de carte est ancrée en bas : la hauteur qu'une carte gagne de sa voisine
  atterrit entre le contenu et la note, où elle se lit comme de l'espacement plutôt que comme
  un bloc vide.
- Le mètre de corrélation L/R prend une teinte progressive de gauche à droite. Réservé à ce
  mètre, où la distance parcourue *est* la lecture : des canaux proches et des canaux quasi
  identiques diffèrent en nature, pas seulement en longueur.
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
