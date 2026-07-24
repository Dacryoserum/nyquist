---
name: dsp-correctness
description: Toucher signal_analysis.rs ou spectral.rs — RMS, peak, true peak, LUFS, dynamic range, clipping, FFT/spectrogramme. Tout calcul numérique sur le signal audio dont un chiffre affiché à l'utilisateur dépend directement.
---

# Justesse du calcul de signal

Nyquist affiche des chiffres (LUFS, DR, true peak...) que des audiophiles vont comparer
directement à ce que montrent foobar2000, RX, ou les plateformes de streaming. Un écart
silencieux discrédite tout l'outil, pas juste la fonction concernée.

## Normes, pas d'improvisation

- LUFS et true peak passent par `ebur128` (voir AGENTS.md) — ne jamais réimplémenter à la
  main le K-weighting, le gating ou l'oversampling. Si `ebur128` ne couvre pas un besoin,
  chercher pourquoi avant de contourner.
- Dynamic range (DR) : préciser explicitement quelle définition est utilisée (DR14 façon
  Pleasurize Music Foundation ≠ dynamic range simple crête/RMS). Documenter le choix dans
  le commit et dans le label affiché à l'utilisateur — ne pas afficher juste « DR » sans
  dire ce que ça mesure.
- Clipping : compter les échantillons à `abs(sample) >= full_scale`, pas une approximation
  à un epsilon arbitraire non documenté.

## Pièges numériques

- Travailler en `f32`/`f64` selon ce que `symphonia` retourne déjà — ne pas introduire de
  conversion supplémentaire qui tronque la précision avant l'analyse.
- Accumuler les moyennes/RMS en `f64` même si le signal source est en `f32`, pour éviter la
  perte de précision sur de longs fichiers.
- Vérifier l'ordre des canaux (interleaved vs planar) explicitement à chaque frontière
  symphonia → analyse : une inversion silencieuse fausse les stats par canal sans crasher.
- Un `cargo clippy` qui râle sur une comparaison de flottants ou un cast ici est
  probablement un vrai bug, pas du bruit à `#[allow]`.

## Auto-review avant de rendre la main

- [ ] Le calcul cite la norme ou la définition exacte qu'il implémente (commentaire ou
      message de commit), pas juste "calcule le RMS".
- [ ] Pas de réimplémentation maison de ce que `ebur128` fait déjà.
- [ ] Testé sur un signal de référence à valeur connue (ex. sinus 1kHz à -3dBFS → RMS
      attendu ≈ -6dB) avant de considérer le calcul correct.
- [ ] Le label affiché à l'utilisateur précise ce que la métrique mesure réellement.
