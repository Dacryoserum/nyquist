# Contributing to Nyquist

Nyquist is an early-stage, MVP-phase project — the architecture can still move, but the
workflow below is fixed from day one.

## Workflow

- `main` is always green and deployable. No direct commits to `main`, ever — always a
  branch + a pull request, even for tiny changes.
- Branch names: `feat/`, `fix/`, `docs/`, `refactor/`, `test/`, `chore/`, `build/`.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/),
  one line in the vast majority of cases:

  ```
  type(scope): short summary, imperative, lowercase, no trailing period
  ```

  Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `build`, `perf`.
  Scopes: `decode`, `signal`, `spectral`, `transcode`, `ui`, `ipc`, `ci`.
  Use `!` after the scope for a breaking change (e.g. `feat(ipc)!: ...`) — this applies to
  any change to the backend↔frontend JSON contract.

- Open a PR with a description of what changed and why. CI must be green before merge.
  Squash-merge by default to keep `main`'s history readable.

## Before opening a PR

Backend (`src-tauri/`):

```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

Frontend:

```bash
npm run check
npm run build
```

Full integration:

```bash
npm run tauri dev
```

## Changes to the signal analysis or transcode detection code

This project's core value is a technical verdict users will trust at face value. Changes
to `signal_analysis.rs`, `spectral.rs`, or `transcode_detect.rs` are held to a higher bar
than typical UI changes — see `.claude/skills/dsp-correctness/SKILL.md` and
`.claude/skills/transcode-heuristic-validation/SKILL.md` for the specifics (cite the norm
you're implementing, validate threshold changes against the fixtures corpus, report
false-positive/negative impact in the PR).

## Changelog

User-facing changes get an entry in `CHANGELOG.md` under `[Unreleased]`, written for a
human, not a diff summary.

## License

By contributing, you agree your contributions are licensed under the project's MIT
license (see `LICENSE`).
