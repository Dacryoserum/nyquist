# La lecture s'arrête avant la fin des fichiers longs

> **Statut : clos en v0.5 — mais pas par ce qui est décrit ici.**
>
> Le problème a fini par disparaître avec son support : la lecture ne passe plus par
> l'élément `<audio>` du tout. `src-tauri/src/player.rs` joue les échantillons décodés par
> l'analyse (rodio/cpal), et le serveur HTTP loopback dont ce document retrace la mise au
> point a été supprimé.
>
> Ce qui a débloqué le diagnostic est arrivé après coup, en cherchant deux *autres* symptômes
> — un seek qui tombait à côté et un compteur incohérent : les trois avaient une cause
> commune, que ce document n'avait pas identifiée. L'élément se forge **sa propre durée** en
> parsant le fichier, pendant que le curseur, l'axe du spectrogramme et le rapport utilisent
> celle du décodeur. Deux horloges. La troncature en était le symptôme le plus visible, pas
> la maladie — et c'est pourquoi améliorer le transport ne l'a jamais réglée.
>
> **La cause racine côté WebKit reste non démontrée**, et le restera : elle n'a plus d'objet.
> Ce qui est mesuré, c'est qu'un transport natif livre un fichier de 40 Mio en entier, ce que
> couvre désormais un test automatisé.
>
> Conservé pour les mesures, qui restent valides et coûteuses à refaire : ce que WebKit
> exige de ses en-têtes, les deux pièges d'instrumentation (volume nul en page masquée,
> `playbackRate` élevé), et les fausses pistes à ne pas reparcourir.

---

## 1. Le symptôme

Dans l'app, un FLAC de 9:49 se lit jusqu'à un point situé bien avant la fin, puis s'arrête.
Aucune erreur n'est levée. Le compteur affiche le temps d'arrêt sur la durée totale correcte
(par ex. « 8:48 / 9:49 »), et le playhead du spectrogramme s'immobilise au même endroit. Le
`<audio>` **révise sa propre durée** jusqu'au point d'arrêt : il croit vraiment que le fichier
finit là. Le point d'arrêt varie d'un essai à l'autre.

### Fichier de reproduction

```
/Users/matthieu/Documents/FLAC/Qobuz Downloads/Ramin Djawadi - Game Of Thrones (Music from the HBO® Series - Season 6) (2016) [16B-44.1kHz]/03. Light of the Seven.flac
```

40 124 885 octets, FLAC 16 bits / 44,1 kHz / stéréo, 25 979 082 échantillons = **589,09 s**.
`ffprobe`, symphonia et `afinfo` (CoreAudio) sont d'accord à la milliseconde. Le chemin
contient espaces, crochets et un `®` — utile, ça exerce l'encodage d'URL.

---

## 2. Ce qui est mesuré et HORS DE CAUSE

Ne pas refaire ces mesures.

| Vérification | Résultat |
|---|---|
| Le fichier lui-même (ffprobe, symphonia, afinfo, structure FLAC) | Irréprochable, 589,09 s partout, 0 erreur de décodage |
| Contenu de la fin | **69 s de musique** à partir de 8:40, RMS −12 à −15 dB. Pas du silence |
| **Fichier entier servi par notre serveur** | **MD5 identique à l'original** (`8ee3a503…`) |
| **Le même fichier réassemblé depuis 39 plages de 1 MiB** | **MD5 identique** |
| Plage ouverte (`bytes=X-`), plage hors bornes (416), HEAD | Conformes |
| Accumulation de connexions / threads bloqués | Aucune : `established=0` sur tous les relevés pendant une lecture complète |
| Bug Tauri sur gros médias ([#6375](https://github.com/tauri-apps/tauri/issues/6375)) | Concerne des fichiers de 3,5 Go+, corrigé. Sans rapport |
| `Failed to acquire RBS assertion 'WebKit Media Playback'` dans les logs | **Bruit** : présent aussi dans les lectures qui vont au bout (binaire non empaqueté) |

**Le serveur ne tronque rien.** Le problème est dans ce que le moteur média de WebKit fait des
octets qu'il reçoit, pas dans les octets.

---

## 3. Fausses pistes écartées (ne pas y revenir)

- **« Le verrou `scrubbing` reste bloqué. »** Infirmé par l'utilisateur : ça arrive sans jamais
  toucher au curseur. Le correctif a quand même été conservé — un seul événement `change`
  manqué gelait l'affichage pour toute la session, c'était une vraie fragilité.
- **« WKWebView plafonne les schémas personnalisés vers 32 MiB, et c'est LA cause. »** Le
  plafond est réel pour `asset://` et un schéma custom (deux arrêts mesurés à 31,85 et
  32,50 MiB), mais ce n'est pas l'explication du même symptôme en HTTP.
- **« `copy_exact` sort en silence sur `read == 0` et sous-livre. »** Mesuré : jamais emprunté,
  et les MD5 le prouvent a posteriori.
- **Arrêts observés dans un banc d'essai WKWebView avec `volume = 0`** : artefact du banc.
  WebKit le dit lui-même dans son journal — `Suspending silent playback after page visibility:
  hidden`. Une lecture silencieuse dans une page masquée est suspendue, ce qui n'a rien à voir
  avec le bug. Toujours tester avec un volume non nul.

---

## 4. Le bug MIME (corrigé, acquis)

Tauri sert les FLAC en `audio/x-flac` (via la crate `infer`), forme legacy que WebKit ne
reconnaît pas. Mesuré en servant le même fichier à Safari, seul le `Content-Type` changeant :

| `Content-Type` | Durée vue par WebKit |
|---|---|
| `audio/flac` | **589,11 s** — correcte |
| `audio/x-flac` | `NaN`, erreur 4 (`MEDIA_ERR_SRC_NOT_SUPPORTED`) |
| `application/octet-stream` | `NaN`, erreur 4 |

Sur un schéma d'URI personnalisé WebKit ne peut pas rejeter : il renifle et **estime** la
durée (521,61 s au lieu de 589,09). Corrigé par `media_server.rs::mime_for`, qui décide par
extension.

---

## 5. Ce qui a été corrigé ensuite

Le point commun des **trois** transports qui ont échoué (`asset://`, schéma custom, et la
première version du serveur HTTP) : aucun ne permettait au webview de **conserver** ce qu'il
avait téléchargé. Or macOS force la page à être économe dès que la fenêtre passe derrière une
autre, et WebKit purge alors son tampon puis redemande. Mesuré dans l'app : elle streame par
plages d'environ 1,4 Mo tout au long du morceau, une trentaine de fois.

- **Les réponses sont cachables** — `Cache-Control: no-store` remplacé par
  `private, max-age=3600` + `ETag`. Sans danger ici : l'URL porte un jeton tiré une fois par
  lancement, donc une URL désigne un fichier tel qu'il était pendant une session.
- **Connexions persistantes** — une trentaine de handshakes TCP par morceau tombe à un ou
  deux. Le cadrage est fait par `Content-Length` sur chaque réponse, jamais en chunked.
- **`HEAD` répondu** (renvoyait `400`) et **garde sur les fichiers vides** (`len - 1` débordait).
- **Aucun timeout ne peut couper une réponse** : le seul timeout (30 s) s'applique entre deux
  requêtes sur une connexion inactive. Une réponse plus courte que son `Content-Length` est
  précisément le signal « la ressource est finie » — en fabriquer serait absurde ici.
- **Filet de sécurité côté app** (`+page.svelte`, `handleEnded`) : on connaît la vraie durée
  par notre propre décodage, donc une fin prématurée est détectable. Quand elle arrive, la
  source est rechargée et la lecture reprend au point d'arrêt. Borné à 5 reprises et toujours
  en avant, pour qu'un fichier qui finit vraiment tôt ne boucle jamais.

---

## 6. Le banc d'essai (`.claude/tools/`)

Reproduire sans l'app et sans clic, en accéléré :

```bash
# 1. Le serveur seul (media_server.rs n'a aucune dépendance hors std)
rustc --edition 2021 -O .claude/tools/serve_main.rs -o /tmp/serve
/tmp/serve "<fichier>"          # imprime l'URL

# 2. Une vraie WKWebView, même moteur que Tauri
swiftc -O .claude/tools/wkplay.swift -o /tmp/wkplay
/tmp/wkplay "<url>" 8 130 0.02  # url, vitesse, timeout, volume
```

⚠️ **Le volume doit être non nul** (cf. §3) et à vitesse ×8 WebKit avale le fichier entier
d'un coup : ça ne reproduit **pas** le régime streaming de l'app. Pour ça, vitesse 1.

**WebKit dit lui-même ce qu'il fait.** C'est l'instrument qui manquait aux deux hypothèses
fausses précédentes :

```bash
log stream --style compact --info --debug \
  --predicate 'process == "nyquist" AND subsystem BEGINSWITH "com.apple.WebKit"'
```

On y lit `HTMLMediaElement::pauseInternal`, `MediaElementSession::beginInterruption`, la
raison en clair d'une suspension, et `updateNowPlayingInfo(… duration = X, now = Y)` toutes
les 5 s — soit la durée et la position, sans instrumenter l'app.

---

## 7. Ce qui reste ouvert

Après les correctifs : **6 lectures complètes sur 6** (3 dans le banc WKWebView, 3 dans l'app
réelle en temps réel, batterie, fenêtre en arrière-plan), durée jamais révisée. Mais
l'utilisateur a observé l'arrêt de façon répétée, et le mécanisme exact côté WebKit n'a pas
été mis en évidence : **on n'a pas prouvé que la cause est morte, on a supprimé les conditions
qui la rendaient probable et posé un filet.**

Si ça récidive :

1. Le bandeau de diagnostic affiche `reprises=N` — si N > 0, le filet a fonctionné et le bug
   est toujours là, seulement masqué. C'est l'information la plus importante à relever.
2. Capturer le journal WebKit (§6) au moment de l'arrêt : il nomme la cause.
3. Piste jamais explorée : le seektable du fichier s'arrête à 580,02 s alors que le fichier va
   à 589,09 s. Un décodeur qui déduirait la fin du dernier point de seek se tromperait — mais
   de 9 s, pas de 111 s. Faible.
4. Piste jamais explorée : tester sous Windows (WebView2 = Chromium). Si le fichier passe, la
   cause est confinée à WebKit.

## 8. À retirer avant tout commit

Le bandeau de diagnostic temporaire dans `src/routes/+page.svelte`, marqué par **trois**
occurrences de `TEMPORARY DIAGNOSTIC` (l'état `diag` + `refreshDiag`, les handlers sur
`<audio>`, le `<pre class="diag">` et son style). Le garder tant que l'utilisateur vérifie :
c'est là qu'apparaît `reprises=N`.

Note : un renommage de module a provoqué une erreur de lien (`symbol(s) not found for
architecture arm64`) due à des artefacts obsolètes. `cargo clean -p nyquist` la résout.
