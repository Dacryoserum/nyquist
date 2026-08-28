# Outils de diagnostic

Vide pour l'instant — et c'est le bon état.

Ce dossier contenait `wkplay.swift` (jouer un fichier dans un `WKWebView` isolé) et
`serve_main.rs` (servir un fichier via le serveur média), deux bancs construits pour
instrumenter la lecture à travers l'élément `<audio>` du webview.

La lecture ne passe plus par le webview : `src-tauri/src/player.rs` joue directement les
échantillons que l'analyse a décodés. Il n'y a plus de transport à instrumenter, plus de
serveur HTTP, et plus de seconde horloge à confronter à la nôtre. Les deux outils n'avaient
plus de sujet.

Voir `.claude/audits/INVESTIGATION-lecture-tronquee.md` pour ce que ces bancs ont établi pendant
qu'ils servaient — les mesures restent valides, seule leur cible a disparu.
