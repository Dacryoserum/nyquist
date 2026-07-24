## What & why

<!-- One or two sentences. Link an issue if there is one. -->

## Changes

<!-- Bullet list of what changed. -->

## Testing

<!-- How did you verify this? Commands run, files tested against. -->
<!-- If this touches transcode_detect.rs / spectral cutoff thresholds: report the
     false-positive/false-negative impact on the fixtures corpus, even approximate. -->

## Checklist

- [ ] `cargo build && cargo test && cargo clippy -- -D warnings` pass (backend changes)
- [ ] `npm run check && npm run build` pass (frontend changes)
- [ ] Commit messages follow Conventional Commits (`type(scope): summary`)
- [ ] Breaking changes to the backend↔frontend JSON contract are marked with `!`
- [ ] `CHANGELOG.md` updated if user-facing
