---
name: tauri-ipc-contract
description: Toucher commands.rs, la forme du JSON retourné au frontend, ou src/lib/api.ts côté Svelte. Le contrat entre le backend Rust et l'UI — le casser silencieusement casse le build ou l'affichage sans erreur claire.
---

# Contrat IPC backend ↔ frontend

Le frontend Svelte dépend de la forme exacte du JSON retourné par les commandes Tauri.
Contrairement à une API HTTP versionnée, ici un changement de forme se propage instantanément
au frontend au prochain build — pas de couche de compatibilité entre les deux.

## Payloads volumineux : jamais en JSON dense

- `spectrogram_data` (ou toute matrice temps/fréquence/intensité) ne doit **jamais** être
  sérialisé tel quel pour un fichier de plusieurs minutes en haute résolution — ça bloque
  la webview côté réception. Downsampler à la résolution d'affichage utile (largeur canvas
  en pixels, pas la résolution FFT native) avant de sérialiser, ou passer par un canal
  binaire Tauri plutôt que par la commande JSON standard.
- Se demander, avant d'ajouter un nouveau champ volumineux au contrat : est-ce que le
  frontend en a besoin en une fois, ou est-ce que ça devrait être streamé/paginé ?

## Analyses longues : jamais synchrones

- Toute commande qui décode/analyse un fichier tourne dans une tâche dédiée (pas sur le
  thread d'event Tauri), avec progression émise via `app_handle.emit` vers le frontend.
  Une commande qui bloque plusieurs secondes gèle l'UI entière, pas juste l'affichage du
  résultat.

## Changer la forme du contrat

- Un changement de champ (renommage, type, nesting) touche `commands.rs` **et**
  `src/lib/api.ts` **et** les composants Svelte qui consomment ce champ dans la même PR —
  jamais un backend en avance sur le frontend.
- Si le changement casse la forme existante, le marquer comme breaking (`!` dans le commit,
  voir AGENTS.md) — pas un simple "ajout".

## Auto-review avant de rendre la main

- [ ] Aucune matrice/donnée volumineuse n'est sérialisée en JSON sans downsampling ou canal
      binaire dédié.
- [ ] Toute commande d'analyse longue est async avec progression, pas bloquante.
- [ ] Backend et frontend (types + composants consommateurs) changés ensemble.
- [ ] `npm run check` et `cargo build` passent tous les deux après le changement.
