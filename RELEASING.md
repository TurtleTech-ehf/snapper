# Releasing snapper

A release fans out across several channels. Some fire automatically from the
tag, some are manual. Every release must either complete **all** channels or
**none**; the recovery rules below exist so a half-published version never
lingers.

## Channel inventory

| Channel | Trigger | Workflow / step |
|---|---|---|
| GitHub Release (binaries, shell/powershell installers) | tag `v*` push | `Release` (cargo-dist) |
| Homebrew tap `TurtleTech-ehf/homebrew-tap` | tag `v*` push | `Release` publish-homebrew-formula job |
| PyPI `snapper-fmt` (sdist + wheels) | tag `v*` push | `Python wheels` publish job (trusted publishing) |
| Docs site (Cloudflare Pages) | push to `main` or tag | `Build and Deploy` |
| conda recipe mirror (`conda/recipe.yaml`) | manual, **after** the tag exists | see "After the tag" |
| VS Code extension (`TurtleTech.snapper`) | manual marketplace publish | `editors/vscode` |
| Obsidian plugin / Word add-in | not published (development preview) | built by `WASM` workflow only |
| npm wrapper (`npm/`) | not published (`"private": true`) | none |

crates.io is **not** a channel; nothing publishes there.

## Before the tag

1. Branch `release/vX.Y.Z` off `main`. Bump the version everywhere it is
   pinned; `scripts/check_release_ready.sh` enumerates every file and fails on
   any mismatch:

   ```sh
   scripts/check_release_ready.sh
   ```

   Leave `conda/recipe.yaml`'s `sha256` untouched — it can only be computed
   after the tag exists (see below). Never write a sha by hand.
2. Add the `## vX.Y.Z` CHANGELOG section.
3. Run the full CI matrix locally-equivalent suite:

   ```sh
   cargo test --features "cli,neural,lsp,watch,pandoc,mcp,wasm"
   cargo test --no-default-features --lib
   ```

4. Open the release PR, wait for **all** checks (CI, Pre-checks, WASM,
   Python wheels, Build and Deploy) to pass, then merge.

## The tag

Tag only the merge commit on `main`, only after every merge-triggered
workflow on `main` is green:

```sh
git switch main && git pull
scripts/check_release_ready.sh   # must pass on the exact commit being tagged
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Never tag a release branch head, and never push the tag while `main` CI is
still running: the tag is consumed immediately by three workflows and cannot
be rebuilt in place.

## After the tag

Watch **all three** tag-triggered workflows to completion; a release is not
done while any of them is running or red:

```sh
gh run list --branch vX.Y.Z
```

Then verify each channel:

```sh
gh release view vX.Y.Z                                  # assets + installers
curl -s https://pypi.org/pypi/snapper-fmt/json | jq -r .info.version
gh api repos/TurtleTech-ehf/homebrew-tap/commits --jq '.[0].commit.message'
```

Refresh the conda recipe from the real tarball and verify:

```sh
curl -fsSL -o /tmp/snapper.tar.gz \
  https://github.com/TurtleTech-ehf/snapper/archive/refs/tags/vX.Y.Z.tar.gz
sha256sum /tmp/snapper.tar.gz     # paste into conda/recipe.yaml
scripts/check_release_ready.sh --post-tag
```

Commit that as `chore(conda): refresh source sha256 for vX.Y.Z archive` on
`main`, then update the conda-forge feedstock with the same sha. Finally
publish the VS Code extension from `editors/vscode` at the same version.

## When something fails mid-release

The decision rule is: **has any channel published?**

- **No channel published yet** (workflows failed or were cancelled before
  their publish steps): cancel the remaining tag runs, delete the tag
  (`git push origin :refs/tags/vX.Y.Z`), fix on a branch, merge, and re-tag
  the same version. A tag no consumer has seen may be recreated; a consumed
  one may not.
- **Any channel published** (a GitHub Release exists, PyPI has the version,
  or the tap formula moved): the version number is burned. Do not delete or
  move the tag. Fix forward, bump to `vX.Y.(Z+1)`, and run the full release
  again so every channel converges on the new version. PyPI in particular
  can never re-accept a version, which is how a partial release becomes
  permanent skew.

Never re-run a publish job against a moved tag, and never leave a release
with some channels on the new version and others on the old one overnight —
either finish it or roll forward.
