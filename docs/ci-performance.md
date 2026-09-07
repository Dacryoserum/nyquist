# CI and packaging performance

Baseline: CI run 33173063920 took 12m55s; release run 33173085929 took 22m00s.
The release's Windows validation used 8m45s for the release-profile test step after
Clippy, then packaging started only after all validation jobs finished. Cache saves
also took up to two minutes.

Changes:

- Build, Clippy and tests use the same optimized dev profile without debug symbols in CI.
  The crate already enables optimization for its DSP loops and dependencies. All tests,
  both operating systems, warnings-as-errors and real build/link checks remain required.
  Shipped installers retain the existing optimized release profile.
- Dependency caches use stable per-platform keys and save only on main. Pull-request and
  tag caches are isolated by GitHub, so uploading them does not warm the next main/tag run.
- Packaging runs alongside validation and uploads only workflow artifacts. A separate
  least-privilege job creates the draft after both platforms and every check pass.
- Branch dispatches exercise packaging without creating a release. To warm release caches
  for subsequent tags, run `gh workflow run release.yml --ref main` before cutting a tag.
  This is optional, costs a packaging run, and is not a promise that cold builds are free.

The first run after a compiler/profile change can still be cold. Compare measured job
durations, not just test execution time; cache download/upload and linking count too.

References: [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html),
[GitHub cache scope](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#restrictions-for-accessing-a-cache),
[rust-cache configuration](https://github.com/Swatinem/rust-cache).
