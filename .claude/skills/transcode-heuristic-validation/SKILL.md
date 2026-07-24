---
name: transcode-heuristic-validation
description: Toucher transcode_detect.rs, un seuil ou une heuristique de spectral cutoff dans spectral.rs, ou le calcul du score de confiance authentique/transcodé. La fonctionnalité la plus risquée du projet — un faux positif accuse à tort un master légitime, un faux négatif valide un fichier menteur.
---

# Détection de transcodage : la partie qui peut se tromper

C'est la fonctionnalité qui justifie le projet, et la plus facile à faire mentir : un
encodeur MP3 bien réglé (LAME V0) peut monter à 19-20kHz, un master original peut être
naturellement pauvre en aigus (genre, mixage, vinyle), et un transcodage soigné peut être
upsamplé puis filtré pour masquer la coupure. Une heuristique non validée produit des
verdicts qui semblent fonctionner en démo et se plantent en usage réel.

## Le corpus avant le réglage

- Aucun changement de seuil ou de logique de score ne se juge « à l'œil » sur un ou deux
  fichiers. `src-tauri/tests/fixtures/corpus/` doit contenir, a minima : plusieurs
  FLAC/WAV authentiques à différents sample rates, plusieurs transcodages connus (MP3→FLAC
  et AAC→FLAC, plusieurs bitrates/encodeurs), et des cas pièges (master naturellement sans
  aigus, fichier réellement upsamplé sans mensonge de source).
- Si le corpus n'existe pas encore ou est trop petit pour le changement en cours, le dire
  explicitement plutôt que de livrer un réglage non testé.

## Le verdict reste prudent

- Toujours trois états (authentique probable / transcodé probable / indéterminé), jamais
  binaire. « Indéterminé » est un résultat légitime, pas un échec à corriger.
- Le score de confiance doit être **traçable** : chaque indicateur qui y contribue doit
  apparaître dans `indicators[]` avec une formulation compréhensible par l'utilisateur, pas
  juste un nombre sorti d'une combinaison opaque.
- Un nouvel indicateur (ex. bruit de quantification) s'ajoute au score de façon visible et
  documentée — pas mélangé silencieusement dans une formule existante.

## Auto-review avant de rendre la main

- [ ] Le changement a tourné sur tout le corpus disponible, pas juste le fichier qui a
      motivé le changement.
- [ ] Taux de faux positifs/négatifs avant/après rapporté dans la description de la PR,
      même approximatif sur un petit corpus.
- [ ] Le verdict reste à 3 états ; aucun chemin ne force un verdict binaire.
- [ ] Chaque indicateur du score est visible et explicable dans `indicators[]`.
- [ ] Si le corpus est insuffisant pour valider le changement, c'est dit explicitement dans
      la PR plutôt que passé sous silence.
