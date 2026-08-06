#!/usr/bin/env bash
# Gate a release before tagging: every channel-pinned version must match
# Cargo.toml, the CHANGELOG must carry the version, and the tag must not
# exist yet. With --post-tag, additionally verify that conda/recipe.yaml
# carries the sha256 of the real published tag tarball.
#
# Usage:
#   scripts/check_release_ready.sh             # pre-tag gate
#   scripts/check_release_ready.sh --post-tag  # after the tag is pushed
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="pre"
if [[ "${1:-}" == "--post-tag" ]]; then
  mode="post"
fi

version="$(sed -n '/^\[package\]$/,/^\[/s/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
if [[ -z "$version" ]]; then
  printf 'could not read version from Cargo.toml\n' >&2
  exit 1
fi
printf 'checking release readiness for v%s\n' "$version"

failed=0
fail() {
  printf 'NOT READY: %s\n' "$1" >&2
  failed=1
}

expect() {
  local pattern="$1" file="$2"
  if ! grep -Fq "$pattern" "$repo_root/$file"; then
    fail "$file is missing '$pattern'"
  fi
}

# Cargo.lock carries the workspace crate at the release version.
if ! grep -A1 '^name = "snapper-fmt"$' "$repo_root/Cargo.lock" \
    | grep -Fq "version = \"$version\""; then
  fail "Cargo.lock snapper-fmt entry is not $version"
fi

# JSON manifests across the wasm package and editor integrations.
json_versioned=(
  npm/package.json
  packages/snapper-wasm/package.json
  packages/snapper-wasm/package-lock.json
  editors/obsidian/package.json
  editors/obsidian/package-lock.json
  editors/obsidian/manifest.json
  editors/word/package.json
  editors/word/package-lock.json
  editors/vscode/package.json
)
for file in "${json_versioned[@]}"; do
  expect "\"version\": \"$version\"" "$file"
done

# Word add-in ships the version in three more places.
expect "$version" editors/word/manifest.xml
expect "$version" editors/word/src/taskpane/taskpane.html
expect "$version" editors/word/README.md
expect "$version" editors/vscode/README.md

# Conda recipe mirror and docs.
expect "version: \"$version\"" conda/recipe.yaml
expect "release = \"$version\"" docs/source/conf.py

# pre-commit / GitHub Action rev pins in user-facing docs.
rev_pinned=(
  README.md
  readme_src.org
  docs/orgmode/tutorials/quickstart.org
  docs/orgmode/howto/ci-enforcement.org
  docs/orgmode/howto/vale-integration.org
)
for file in "${rev_pinned[@]}"; do
  expect "rev: v$version" "$file"
done
expect "TurtleTech-ehf/snapper@v$version" docs/orgmode/howto/ci-enforcement.org

# The list above only proves the current pin is present somewhere in each
# file. Catch the other half: no prose doc may point a reader at a rev or a
# release download for any other version. A snippet pinned at an older tag
# resolves against that tag's hook definitions and installer assets.
stale_pins="$(
  git -C "$repo_root" ls-files -- '*.md' '*.org' ':!CHANGELOG.md' \
  | while read -r doc; do
      awk -v ver="v$version" -v f="$doc" '
        /repo:[[:space:]]*https:\/\/github\.com\/TurtleTech-ehf\/snapper/ { ours = 1; next }
        /repo:[[:space:]]*https:\/\/github\.com\// { ours = 0 }
        ours && /rev:[[:space:]]*v[0-9]+\.[0-9]+\.[0-9]+/ && index($0, ver) == 0 { print f ":" FNR ":" $0 }
        /snapper\/releases\/(tag|download)\/v[0-9]+\.[0-9]+\.[0-9]+/ && index($0, ver) == 0 { print f ":" FNR ":" $0 }
      ' "$repo_root/$doc"
    done
)"
if [[ -n "$stale_pins" ]]; then
  fail "docs pin a version other than v$version:"$'\n'"$stale_pins"
fi

# CHANGELOG must document this release.
expect "## v$version" CHANGELOG.md

# The public availability contract must hold too.
bash "$repo_root/scripts/check_public_contract.sh" || fail "check_public_contract.sh failed"

if [[ "$mode" == "pre" ]]; then
  if [[ -n "$(git -C "$repo_root" tag -l "v$version")" ]]; then
    fail "local tag v$version already exists; a consumed tag must never move"
  fi
  if git -C "$repo_root" ls-remote --tags origin "refs/tags/v$version" | grep -q .; then
    fail "remote tag v$version already exists; bump the version instead"
  fi
  if [[ -n "$(git -C "$repo_root" status --porcelain)" ]]; then
    fail "working tree is dirty"
  fi
else
  # Post-tag: the conda recipe sha256 must be computed from the real
  # tarball, never written by hand.
  url="https://github.com/TurtleTech-ehf/snapper/archive/refs/tags/v$version.tar.gz"
  tarball="$(mktemp)"
  trap 'rm -f "$tarball"' EXIT
  if ! curl -fsSL --retry 3 -o "$tarball" "$url"; then
    fail "could not download $url (tag not pushed yet?)"
  else
    actual="$(sha256sum "$tarball" | cut -d' ' -f1)"
    if ! grep -Fq "sha256: $actual" "$repo_root/conda/recipe.yaml"; then
      fail "conda/recipe.yaml sha256 does not match the v$version tarball ($actual)"
    fi
  fi

  # crates.io serves the install path the docs hand to users, so it must
  # carry this version like every other channel.
  if crates_json="$(curl -fsSL --retry 3 -A 'snapper-release' \
      https://crates.io/api/v1/crates/snapper-fmt)"; then
    crates_version="$(printf '%s' "$crates_json" | tr ',' '\n' \
      | sed -n 's/.*"max_version":"\([^"]*\)".*/\1/p' | head -n 1)"
    if [[ "$crates_version" != "$version" ]]; then
      fail "crates.io serves snapper-fmt $crates_version, not $version"
    fi
  else
    fail "could not reach crates.io to verify the published version"
  fi
fi

if [[ "$failed" -eq 0 ]]; then
  printf 'release checks passed for v%s (%s-tag)\n' "$version" "$mode"
fi
exit "$failed"
