---
name: release-packaging
description: Toucher tauri.conf.json, les versions dans Cargo.toml/package.json, la CI/CD (.github/workflows), ou tout ce qui touche à la signature/notarization macOS. Le packaging d'une app desktop distribuée à des inconnus, pas un service interne.
---

# Packaging et distribution

Contrairement à un service web, un binaire desktop mal signé ou mal versionné bloque
l'utilisateur avant même qu'il voie l'app — pas de rollback serveur possible, l'utilisateur
a déjà téléchargé le mauvais artefact.

## macOS d'abord

- Un `.dmg` non signé/notarié déclenche l'avertissement Gatekeeper « développeur non
  identifié ». Avant V1.0, décider explicitement : notarization payante (compte développeur
  Apple) ou documentation claire du contournement pour les early adopters — ne pas laisser
  ça flou dans une release publique. Voir `.claude/CONTEXT.md`.
- Vérifier que la target CI (`tauri-action` ou équivalent) génère bien un `.dmg` universel
  (Intel + Apple Silicon) ou deux artefacts séparés clairement nommés — ne pas livrer un
  binaire qui ne tourne que sur l'architecture du runner CI par défaut.

## Versions

- Version cohérente entre `Cargo.toml` (src-tauri), `package.json` (frontend), et
  `tauri.conf.json` — les trois doivent bouger ensemble dans la même PR, jamais un seul.
- Une entrée `CHANGELOG.md` accompagne toute release taggée, écrite pour un utilisateur qui
  ne lit pas le code.

## Auto-review avant de rendre la main

- [ ] Version alignée dans les 3 fichiers (Cargo.toml, package.json, tauri.conf.json).
- [ ] Le statut de signature/notarization de l'artefact produit est explicite dans la PR ou
      la release, pas implicite.
- [ ] CHANGELOG.md mis à jour si la CI produit une release taggée.
