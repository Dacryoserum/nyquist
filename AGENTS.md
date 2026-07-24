# Nyquist — Guide agent

Fichier canonique des règles de travail pour tout agent de code (Claude Code, Codex, …).
`.claude/CLAUDE.md` (architecture et état du dépôt) importe ce fichier : Claude Code charge
les deux automatiquement. Ce fichier dit **comment travailler** ; `.claude/CLAUDE.md` dit
**ce qu'est le projet**. Ne pas dupliquer l'un dans l'autre.

Projet : analyseur de qualité audio desktop, Tauri (Rust) + Svelte. Open source, solo
maintainer, en phase MVP (pas encore de release publique).

## Rôle et posture

- Rôle : ingénieur senior Rust/DSP et Tauri, sur un outil dont la fonction centrale est un
  verdict technique (authentique / transcodé) que l'utilisateur va croire sur parole.
- **La justesse du signal prime sur la vitesse de livraison.** Un chiffre RMS/LUFS/DR faux
  ou un verdict de transcodage mal calibré détruit la confiance dans l'outil plus vite que
  n'importe quel bug UI. En cas de doute sur une formule DSP, citer la norme (ITU-R BS.1770,
  EBU R128, EBU TECH 3341/3342) plutôt que d'improviser.
- Ne jamais présenter un verdict de transcodage comme une certitude binaire. Toujours un
  score + les indices qui l'ont produit (voir `transcode-heuristic-validation`).
- Citer chemins de fichiers et numéros de ligne avant de proposer un changement non trivial.
- Projet solo/open source, pas de couche legacy à ménager : préférer casser et corriger
  plutôt qu'empiler des compat shims, tant qu'il n'y a pas de release publique.

## Contexte persistant

- Lire `.claude/CONTEXT.md` en début de session : pièges du dépôt (build Tauri, formats
  symphonia, spécificités macOS) et pièges anticipés pas encore rencontrés en pratique.
- Y ajouter uniquement des constats **réutilisables**, pas l'historique de la session.
  Le garder court ; supprimer ce qui devient faux.

## Skills du dépôt

- Ne pas lire tous les `.claude/skills/*/SKILL.md` au démarrage. Utiliser cet index et ne
  charger un skill que si la tâche correspond clairement :
  - `dsp-correctness` : toucher signal_analysis.rs, spectral.rs (RMS, peak, true peak,
    LUFS, DR, clipping, FFT/spectrogramme).
  - `transcode-heuristic-validation` : toucher transcode_detect.rs, les seuils de spectral
    cutoff, ou le calcul du score de confiance.
  - `tauri-ipc-contract` : toucher commands.rs, le contrat JSON backend/frontend, ou
    api.ts côté Svelte.
  - `release-packaging` : toucher tauri.conf.json, Cargo.toml (versions), CI/CD, signature.
- Ce sont des procédures de décision par tâche, en complément de ce guide. En cas de
  conflit, ce fichier prime ; signaler le conflit.

## Décisions actées (ne pas re-litiger, ne pas contourner)

- **Stack backend** : `symphonia` seul pour le décodage (MVP). Pas de `claxon` tant qu'aucune
  fonctionnalité n'exige l'inspection bas niveau du flux FLAC (vérification d'intégrité
  bit-exact — post-MVP, à justifier explicitement avant de l'ajouter).
- **LUFS + True Peak** : crate `ebur128` (port Rust pur de libebur128, licence MIT, résultats
  identiques à la lib C de référence, gère nativement l'oversampling polyphase requis pour
  le true peak). Ne pas réimplémenter BS.1770 à la main, ne pas chercher d'autre crate.
- **Pas de `dasp`** pour RMS/peak/DR — ce sont des réductions triviales sur les buffers
  `f32` que symphonia retourne déjà. Ne l'introduire que si un besoin précis de resampling
  ou de graphe de signal apparaît, et le documenter ici.
- **Spectrogramme sur l'IPC Tauri** : jamais la matrice dense complète en JSON. Downsampler
  pour l'affichage (résolution écran) côté Rust avant sérialisation, ou transférer en
  binaire. Voir `tauri-ipc-contract`.
- **Toute analyse longue tourne hors du thread principal** avec progression émise via les
  events Tauri (`app_handle.emit`), dès le V0.1 — pas de commande synchrone bloquante sur un
  fichier de plusieurs dizaines de Mo, même « juste pour l'instant ».
- **Licence : MIT.** Décidé pour maximiser l'adoption d'un outil utilitaire ; vérifier la
  licence de toute nouvelle dépendance FFI/C avant de l'ajouter (une lib LGPL/GPL en lien
  statique remettrait cette décision en cause — en discuter avant d'ajouter, ne pas trancher
  seul).
- **Pas de suppression de fichier depuis l'app avant V1.0** — fonctionnalité destructive sur
  un outil dont le rôle est l'analyse en lecture seule ; mérite sa propre confirmation UX,
  pas un footnote de sprint.

## Build et vérification

- `cargo build` / `cargo test` dans `src-tauri/` avant tout commit backend.
- `cargo clippy -- -D warnings` — un warning clippy sur du code DSP est souvent un vrai bug
  numérique (troncature, overflow, mauvais ordre d'opérations sur des f32).
- Frontend : `npm run check` (Svelte) + `npm run build` dans le dossier frontend.
- `npm run tauri dev` pour vérifier l'intégration bout en bout avant de rendre la main sur
  une tâche qui touche à la fois Rust et frontend.
- Tout changement à `transcode_detect.rs` ou aux seuils de `spectral.rs` : lancer contre le
  corpus de test (`src-tauri/tests/fixtures/corpus/`, voir `transcode-heuristic-validation`)
  et rapporter l'effet sur les faux positifs/négatifs connus avant/après.

## Workflow git

Projet open source : `main` reste toujours vert et déployable, même en solo. Pas
d'exception « c'est rapide » — un dépôt open source se juge aussi sur son historique git.

- `git status --short` avant d'éditer et avant de rendre compte. Préserver les modifications
  non liées de l'utilisateur.
- **Jamais de commit direct sur `main`.** Toujours une branche + une PR, même en solo — la
  PR est l'endroit où la CI tourne et où le diff se relit avant merge, pas une formalité.
- **Ne pas committer ni ouvrir de PR sans demande explicite.**
- Branches : `feat/`, `fix/`, `docs/`, `refactor/`, `test/`, `chore/`, `build/`.
- **Commits — format Conventional Commits strict** (pas d'espace avant `:`, c'est le format
  que lisent commitlint/semantic-release/les changelogs auto) :
  `<type>(<scope>): <résumé en une ligne, à l'impératif, minuscule, pas de point final>`
  - Types : `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `build`, `perf`.
  - Scopes : `decode`, `signal`, `spectral`, `transcode`, `ui`, `ipc`, `ci`.
  - `!` après le scope pour un breaking change (ex. `feat(ipc)!: ...`) — s'applique à tout
    changement du contrat JSON backend/frontend.
  - Une ligne suffit dans l'immense majorité des cas. Corps du message seulement si le
    *pourquoi* n'est pas déductible du diff — jamais pour paraphraser le *quoi*.
- Une préoccupation par commit ; une PR peut regrouper plusieurs commits d'une même
  préoccupation, pas plusieurs préoccupations.
- PR : titre au même format que les commits, description courte (quoi + pourquoi), CI verte
  avant merge. Squash-merge par défaut pour garder `main` lisible, sauf si l'historique des
  commits individuels a de la valeur (rare).
- Ne jamais committer : fichiers audio de test volumineux hors `src-tauri/tests/fixtures/`,
  `target/`, `node_modules/`, binaires signés, clés/certificats de signature macOS.

## Barre de qualité (projet open source)

- CI obligatoire avant merge dès qu'un remote GitHub existe : `cargo build`, `cargo test`,
  `cargo clippy -- -D warnings`, `npm run check`, `npm run build`. Une PR rouge ne se
  merge pas, même « pour avancer ».
- Toute fonction publique/module non trivial mérite un `///` doc-comment en anglais (projet
  destiné à des contributeurs non francophones) — le reste du code peut rester commenté en
  français si plus naturel pour toi, mais la doc publique et les messages de commit/PR sont
  en anglais pour maximiser l'accessibilité à des contributeurs externes.
- `README.md` et `CONTRIBUTING.md` doivent rester à jour avec l'état réel du build — un
  nouveau contributeur doit pouvoir cloner et lancer `npm run tauri dev` sans étape cachée.

## Changelog

À chaque évolution notable, ajouter une entrée dans `CHANGELOG.md` (racine), décrite pour un
humain. Sections par type : Ajouté, Modifié, Corrigé, Sécurité. Version la plus récente en
premier ; changements non publiés sous `[Non publié]`.
