# Nyquist - Audit V2 et plan de correction

Date : 2026-08-26

## Objectif

Ce document est destiné à une IA chargée de corriger le dépôt. Il décrit l'état réel observé
entre `HEAD` (`v0.4.0`) et le worktree non committé, principalement le remplacement de
`asset://` par `src-tauri/src/media_server.rs` et l'ajout d'un mécanisme de reprise dans
`src/routes/+page.svelte`.

Ne pas considérer le problème WebKit comme définitivement expliqué. Les mesures disponibles
montrent que le serveur peut livrer le fichier entier, mais elles ne prouvent pas la cause racine
de l'arrêt observé dans WebKit.

## Règles de travail

- Inspecter `git status --short` avant toute modification et préserver les changements non liés.
- Ne pas supprimer ou réinitialiser des changements existants.
- Ne pas committer ni ouvrir de PR sans demande explicite.
- Charger `tauri-ipc-contract` avant de modifier `commands.rs`, `api.ts` ou la forme d'une commande.
- Charger `release-packaging` avant de modifier la configuration ou le support de plateforme.
- Ne pas laisser de diagnostic temporaire, chemin absolu local ou banc cassé dans le résultat final.
- Ne pas modifier les seuils DSP ou le verdict de transcodage sans relancer le corpus et documenter
  les faux positifs et faux négatifs avant/après.
- Ne pas présenter une reprise automatique comme une preuve que la cause WebKit est corrigée.

## Verdict de l'audit

Le chemin nominal du serveur est cohérent : MIME FLAC explicite, `Range`, `HEAD`,
`Content-Length`, keep-alive et allowlist sont présents. Les tests nominaux passent.

Le lot n'est toutefois pas prêt à être intégré. Il introduit un serveur HTTP écrit à la main,
une surface de sécurité et plusieurs états asynchrones frontend sans les tests de robustesse
correspondants. Il contient aussi une régression de portabilité et des affirmations documentaires
plus fortes que les preuves disponibles.

## Priorité 1 : bloqueurs à corriger

### 1. Démarrage Windows impossible

**Fichiers :** `src-tauri/src/media_server.rs:387-392`, `src-tauri/src/lib.rs:20-24`,
`.github/workflows/release.yml:63-69`.

`random_token()` ouvre `/dev/urandom` en dur et panique en cas d'échec. Le workflow construit
et publie pourtant une version Windows. Le serveur doit utiliser une source CSPRNG portable,
par exemple une dépendance maintenue dont la licence est vérifiée, ou des implémentations
`cfg` propres à chaque plateforme.

Le démarrage ne doit pas transformer un échec récupérable en panic sans message utile. Si le
bind ou la génération du token échoue, retourner une erreur exploitable par l'application ou
afficher un état de démarrage explicite.

**Tests attendus :** compilation et démarrage sur Windows CI, génération de plusieurs tokens
différents, absence de chemin Unix en code non conditionné.

### 2. Limiter la surface de déni de service du serveur

**Fichiers :** `src-tauri/src/media_server.rs:166-176`, `193-203`, `327-375`.

Le serveur crée un thread par connexion et `read_line()` accepte des lignes et headers de taille
illimitée. Un processus local peut ouvrir beaucoup de sockets ou envoyer une requête lente et
énorme avant que le token soit vérifié. Le timeout de lecture de 30 secondes ne limite pas la
mémoire d'une ligne qui continue d'arriver.

Implémenter une politique explicite :

- limite de connexions simultanées ou pool de workers borné ;
- taille maximale de la request-line et des headers ;
- rejet propre des requêtes trop grandes ou incomplètes ;
- timeout d'inactivité entre requêtes conservé, sans couper une réponse valide en cours ;
- politique documentée pour un client qui cesse de lire et bloque les écritures ;
- aucun thread ou buffer non borné contrôlable par un client local.

Le token reste nécessaire pour l'autorisation de fichier, mais il ne protège pas contre la
consommation de ressources avant authentification.

**Tests attendus :** connexions inactives simultanées, request-line très longue, headers très
longs, client qui n'accuse jamais réception, dépassement de la limite et récupération après
rejet.

### 3. Rendre l'autorisation atomique par rapport au fichier servi

**Fichiers :** `src-tauri/src/media_server.rs:102-125`, `226-235`.

Le chemin est canonicalisé et comparé à l'allowlist, puis ouvert dans une opération séparée.
Un remplacement de symlink entre les deux opérations peut faire servir une autre cible que celle
contrôlée.

Préférer une ressource média opaque autorisée une seule fois : ouvrir le fichier au moment de
l'autorisation, conserver une ressource stable ou un handle adapté aux lectures par offset, puis
servir cette ressource par un identifiant aléatoire. Ne pas mettre le chemin filesystem dans
l'URL si cela n'est plus nécessaire. Si une réouverture par chemin reste retenue, documenter et
tester une vérification d'identité de fichier adaptée à macOS et Windows ; une simple nouvelle
canonicalisation n'est pas atomique.

**Tests attendus :** remplacement de symlink après autorisation, remplacement du fichier pendant
une lecture, chemin supprimé, plusieurs ressources autorisées simultanément.

### 4. Aligner le MIME sur le média réellement analysé

**Fichiers :** `src-tauri/src/media_server.rs:128-145`, `src-tauri/src/decode.rs:47-54`,
`src/routes/+page.svelte:147-170`.

Symphonia peut reconnaître le contenu même si l'extension est absente ou fausse, tandis que le
serveur choisit le MIME par extension. Le commentaire du serveur affirme à tort que les deux
ensembles sont identiques. `.opus` n'est pas géré par `mime_for()` et n'est pas proposé dans le
sélecteur, alors que le backend connaît Opus.

Choisir une seule source de vérité :

- soit passer le conteneur/codec et le MIME déterminés par l'analyse à l'autorisation ;
- soit détecter le format au moment de l'ouverture avec une table validée ;
- soit stocker le MIME dans la ressource autorisée ouverte après analyse.

Mettre à jour ensemble la commande Tauri, `src/lib/api.ts` et les consommateurs Svelte si la
signature change. Couvrir au minimum FLAC, MP3, WAV, M4A/ALAC, AAC brut, OGG et Opus, ainsi que
les extensions en majuscules, absentes ou incorrectes.

### 5. Éliminer les courses entre analyses

**Fichier :** `src/routes/+page.svelte:122-142`.

`analyze()` peut être lancée par plusieurs drops ou sélections. Une ancienne Promise peut alors
écrire `result`, `audioSrc`, `error` ou `loading` après la nouvelle analyse.

Ajouter un identifiant de génération monotone ou un token d'annulation. Chaque continuation doit
vérifier qu'elle correspond encore à la demande courante avant d'écrire l'état. Le `finally` doit
également être conditionné par cette génération. Les callbacks du lecteur et de la reprise
doivent vérifier la même génération avant de modifier l'interface.

Réinitialiser proprement l'ancien élément audio avant de remplacer la source : `pause()`, remise
à zéro si possible, puis démontage. Ne pas laisser un ancien élément émettre des événements sur
le nouveau résultat.

**Tests attendus :** deux analyses avec réponses dans l'ordre inverse, analyse rapide suivie
d'une analyse lente, autorisation réussie mais analyse échouée, et analyse réussie mais lecture
indisponible.

### 6. Séparer analyse, autorisation et erreur de lecture

**Fichiers :** `src/routes/+page.svelte:134-142`, `175-178`, `201-218`, `796-818`.

`Promise.all([analyzeFile(), authorizePlayback()])` masque le rapport si l'autorisation échoue.
Inversement, `play()` est appelé sans attendre ni capturer son rejet. `onerror` ne fait qu'écrire
un diagnostic temporaire. La reprise fait pareil dans `loadedmetadata`.

Afficher le rapport dès que l'analyse réussit, avec un `playbackError` séparé si l'audio ne peut
pas être servi ou lu. Capturer les rejets de `play()` et traduire un message utilisateur utile.
Gérer au minimum les événements `error`, `stalled`, `waiting` et les échecs de chargement sans
les confondre avec une fin naturelle.

La reprise doit :

- vérifier que l'élément et la génération sont toujours courants ;
- capturer le rejet de la nouvelle lecture ;
- rester bornée ;
- conserver la position de façon sûre ;
- afficher clairement si elle a dû intervenir ;
- ne pas masquer une erreur persistante sous une apparence de lecture active.

### 7. Corriger la validation des ranges HTTP

**Fichiers :** `src-tauri/src/media_server.rs:255-275`, `414-425`.

`bytes=100-50` est accepté, puis `end - start + 1` peut provoquer un panic en debug ou une
longueur incohérente en release. Le parseur confond aussi certaines ranges malformées avec une
range ouverte.

Utiliser un résultat de parsing distinguant au moins : absence de Range, range valide et range
invalide. Rejeter `end < start` par `416 Range Not Satisfiable`. Implémenter les ranges suffixes
ou les rejeter explicitement par `416`, mais ne pas les interpréter silencieusement comme une
réponse complète sans décision documentée. Vérifier les bornes, les débordements et la cohérence
de `Content-Range`/`Content-Length`.

**Tests attendus :** `0-0`, `len-1-len-1`, `start >= len`, range ouverte, suffixe, inversée,
multi-range, caractères invalides, fichier vide et `HEAD` avec et sans range.

## Priorité 2 : exactitude et maintenance

### 8. Corriger la sémantique du cache

**Fichiers :** `src-tauri/src/media_server.rs:183-190`, `239-244`, `287-296`.

La même URL est produite pour un même chemin pendant toute la session et reste fraîche une heure.
Si le fichier est remplacé au même chemin, l'analyse peut montrer le nouveau contenu tandis que
WebKit lit une ancienne réponse en cache. L'ETag taille + mtime à la seconde n'empêche pas ce
problème, notamment avec `max-age`.

La solution préférée est une URL opaque nouvelle pour chaque snapshot/autorisation, associée à
un fichier déjà ouvert ou à une identité vérifiée. Une ressource immuable peut alors être servie
avec un cache long. Sinon utiliser une validation conditionnelle correcte (`If-None-Match` /
`304`) et une politique qui force la revalidation. Ajouter un test de remplacement au même chemin
et de réanalyse immédiate.

### 9. Corriger le scrubber sans régression clavier

**Fichier :** `src/routes/+page.svelte:354-358`, `759-772`.

Le nouveau code n'active `scrubbing` que sur `pointerdown`. Une navigation au clavier déclenche
`input` sans activer ce verrou, donc `timeupdate` peut déplacer le curseur pendant l'appui sur
les flèches. Un listener sur `window` ne garantit pas non plus de recevoir un `pointerup` après
sortie de la fenêtre.

Activer le mode de scrubbing sur `keydown` et le libérer sur `keyup`, `pointerup`, `pointercancel`
et perte de focus. Utiliser la capture de pointeur si nécessaire. Tester pointeur, clavier,
annulation OS et sortie de fenêtre.

### 10. Supprimer les diagnostics temporaires avant livraison

**Fichier :** `src/routes/+page.svelte:45-60`, `791-794`, `1551-1567`.

Le bandeau `diag` est explicitement temporaire, mais il est encore rendu. Il n'est pas vidé lors
d'une nouvelle analyse et peut afficher un état du fichier précédent.

Le supprimer avant intégration. Si un instrument reste nécessaire, le placer derrière un mode de
développement explicite, sans l'inclure dans le build de production et sans le présenter comme
un message utilisateur normal.

### 11. Rendre les tests réellement probants

Les tests actuels du serveur vérifient surtout des buffers synthétiques de quelques KiB. Ils ne
testent pas le chemin complet `<audio>`/WKWebView, le passage en arrière-plan, les erreurs réseau,
la concurrence ou les modifications du fichier.

Ajouter :

- tests unitaires et socket pour toutes les bornes HTTP listées ci-dessus ;
- test de réponse complète sur un fichier nettement supérieur à 32 MiB ;
- test de fermeture client pendant une réponse ;
- test de cache et remplacement de fichier ;
- test d'analyse concurrente côté frontend ;
- test de rejet de `HTMLMediaElement.play()` et de l'événement `error` ;
- test Windows de compilation et de démarrage ;
- banc WKWebView reproductible documentant volume non nul, vitesse réelle, fenêtre masquée et
  code de sortie d'erreur.

`.claude/tools/serve_main.rs:1` contient un chemin absolu propre à un poste et doit devenir
portable. `.claude/tools/wkplay.swift:31-33` quitte avec le code 0 sur une erreur, ce qui empêche
de l'utiliser comme test automatisé. Les erreurs doivent produire un code non nul.

Une lecture manuelle réussie n'est pas une preuve suffisante. Le résultat attendu doit préciser
si une reprise a eu lieu. Une lecture complète avec `reprises=1` signifie que le défaut existe
encore et a seulement été masqué.

### 12. Mettre la documentation en cohérence

Mettre à jour :

- `README.md:53-55` : le verdict comporte quatre états, pas trois ;
- `README.md:67-69` : la lecture passe par le serveur loopback, pas `asset://` ;
- `src-tauri/Cargo.toml:25-27` : commentaire `protocol-asset` obsolète ;
- `CHANGELOG.md:11-25` : ne pas présenter la cause WebKit comme démontrée ;
- `.claude/INVESTIGATION-lecture-tronquee.md` : conserver la distinction entre mesures prouvées,
  hypothèses et workaround ;
- `AUDIT_COMPLET.md` : ce fichier est obsolète. Il cite `media_protocol.rs`, décrit un mismatch
  `protocol-asset` désormais corrigé et dit que les tests sont bloqués alors qu'ils passent.
  Le mettre à jour ou le remplacer, mais ne pas le laisser comme référence concurrente.

Le commentaire « le serveur sert exactement les extensions acceptées par analyze_file » doit être
retiré ou rendu vrai. Toute limite de format doit être visible dans le README et l'interface.

### 13. Nettoyer le formatage et la taille du changement

`media_server.rs` fait 679 lignes, dont beaucoup de documentation et de tests. Ce nombre n'est
pas à lui seul un défaut : `Range`, cache, keep-alive, sécurité et tests justifient une partie de
la complexité. Le vrai problème est d'avoir une implémentation HTTP artisanale longue sans
bornes, fuzzing ni test d'intégration WebKit.

Après correction :

- exécuter `cargo fmt --all -- --check` et corriger au moins le nouveau module ;
- supprimer les lignes et commentaires devenus faux ou redondants ;
- éviter les chemins absolus dans les outils ;
- conserver une séparation nette entre serveur, logique de ressource et tests ;
- ne pas ajouter de couche de compatibilité inutile avant une release publique.

## Problèmes produit préexistants à ne pas perdre

Ils ne sont pas causés par le serveur média, mais un audit complet ne doit pas déclarer Nyquist
fiable tant qu'ils restent actifs.

### Verdict de transcodage trop affirmatif

**Fichier :** `src-tauri/src/transcode_detect.rs:505-540`.

L'absence de coupure spectrale retourne `ProbablyAuthentic`, alors que le corpus contient des
transcodages MP3 V0 réels sans coupure détectable. Ce résultat doit devenir `Indeterminate`, ou
être reformulé strictement comme « aucun indice détecté », sans cautionner l'authenticité.

### Décodage incomplet encore verdictable

**Fichiers :** `src-tauri/src/decode.rs:89-109`, `src-tauri/src/analysis.rs:148-156`.

Les paquets illisibles sont parfois ignorés et `ResetRequired` termine le décodage sans marquer
le rapport comme incomplet. Les mesures restantes peuvent quand même produire un verdict normal.

Ajouter un statut explicite de couverture/décodage complet. Conserver les mesures disponibles,
mais forcer le verdict à `Indeterminate` avec une explication si le signal est incomplet.

### Autres points DSP hérités à revalider

Avant de modifier les seuils, relire et actualiser les findings correspondants dans l'ancien
rapport :

- bande passante repliée sur Nyquist dans `spectral.rs` et `sample_rate.rs` ;
- LUFS multicanal sans layout dans `decode.rs` et `signal_analysis.rs` ;
- seuil de clipping fixe 16 bits dans `signal_analysis.rs` ;
- true peak et erreurs `ebur128` dans `signal_analysis.rs` ;
- détection bit-depth influencée par les silences dans `bit_depth.rs` ;
- LRA des fichiers courts et `effectively_mono` dans `signal_analysis.rs`/`stereo.rs` ;
- pic mémoire structurel du pipeline complet dans `decode.rs`, `analysis.rs` et `spectral.rs`.

Chaque correction DSP doit être accompagnée d'un test numérique ou d'une comparaison à une
référence documentée. Ne pas traiter ces problèmes en baissant simplement des seuils.

## Définition de fini

Le travail de correction n'est terminé que si toutes les conditions suivantes sont vraies :

- aucune URL média ne permet de sélectionner un chemin arbitraire ou une ressource non autorisée ;
- le serveur démarre sur macOS et Windows ;
- les requêtes malformées ne provoquent ni panic, ni allocation non bornée, ni réponse incohérente ;
- un fichier supporté par l'analyse possède un MIME de lecture validé ;
- deux analyses concurrentes ne peuvent pas mélanger leurs résultats ou leurs lecteurs ;
- une erreur audio est visible comme erreur de lecture, sans effacer un rapport valide ;
- la reprise est bornée, observable et ne sert pas à prétendre que la cause WebKit est prouvée ;
- aucun diagnostic temporaire ou chemin absolu de développeur ne reste dans la livraison ;
- les tests couvrent les bornes HTTP et les scénarios d'échec ;
- `cargo build --locked` passe dans `src-tauri/` ;
- `cargo test --locked --release -- --nocapture` passe dans `src-tauri/` ;
- `cargo clippy --locked --all-targets -- -D warnings` passe ;
- `cargo fmt --all -- --check` passe ;
- `npm run check` passe ;
- `npm run build` passe ;
- `npm run tauri build -- --debug` passe sur macOS ;
- le workflow Windows passe ;
- une lecture réelle d'un FLAC de plus de 32 MiB va au bout au premier essai, en fenêtre active
  et masquée, avec les logs WebKit conservés si le problème réapparaît.

## Commandes de vérification locales

```bash
npm run check
npm run build
npm run tauri build -- --debug

cd src-tauri
cargo build --locked
cargo test --locked --release -- --nocapture
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
```

Ne pas conclure à une correction complète uniquement parce que les commandes nominales passent :
la validation finale doit inclure les tests négatifs et le scénario WebKit réel.
