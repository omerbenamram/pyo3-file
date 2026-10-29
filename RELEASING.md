# Releasing

`pyo3-file` is published to crates.io. Release notes live on the GitHub releases.

## One-time setup

- A crates.io token in the `CRATES_IO_TOKEN` Actions secret.

## Cut a release

1. Pick the version. While on 0.x, a breaking change (a new major `pyo3` counts) or a new
   feature bumps the minor version and fixes alone bump the patch version.
2. Open a `chore(release): X.Y.Z` PR that bumps `version` in `Cargo.toml` and refreshes
   `Cargo.lock`.
3. Merge it.

The merge runs `.github/workflows/auto-tag.yml`. It sees a `Cargo.toml` version with no tag,
pushes the `vX.Y.Z` tag on the merge commit, and calls `.github/workflows/release.yml`, which
publishes the crate and then creates the GitHub release with generated notes.

Pushing a `vX.Y.Z` tag by hand still runs `release.yml` directly; auto-tag then skips that
version because the tag exists. To retry a failed release, re-run the failed jobs of its run.
