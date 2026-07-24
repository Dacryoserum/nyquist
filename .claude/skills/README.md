# Nyquist : skills Claude Code

Skills projet pour Claude Code. Claude les découvre automatiquement et en charge un quand
la tâche correspond à sa `description` ; on ne les invoque pas à la main. Ils encodent le
jugement propre à ce projet : ce qui rend un chiffre DSP faux, ce qui rend un verdict de
transcodage non fiable, ce qui casse le contrat IPC, ce qui bloque un packaging macOS.

| Skill | Se charge quand on… |
|---|---|
| `dsp-correctness` | touche signal_analysis.rs ou spectral.rs (RMS, peak, true peak, LUFS, DR, clipping, FFT) |
| `transcode-heuristic-validation` | touche transcode_detect.rs, un seuil de spectral cutoff, ou le calcul du score de confiance |
| `tauri-ipc-contract` | touche commands.rs, le contrat JSON, ou api.ts côté frontend |
| `release-packaging` | touche tauri.conf.json, les versions Cargo.toml/package.json, la CI/CD, la signature |

`AGENTS.md` reste le socle toujours chargé ; ces skills sont la couche de jugement qui se
charge à la demande.

## Maintenance

Éditer le `SKILL.md` de chaque dossier. Rester dense : ne pas répéter `AGENTS.md` ni
`.claude/CLAUDE.md`, ajouter le jugement qu'ils ne peuvent pas porter. La qualité du
déclenchement vit dans le champ `description` : il doit nommer les situations concernées
avec les mots qu'une tâche réelle emploierait.
