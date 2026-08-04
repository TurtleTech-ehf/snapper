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
)
for file in "${rev_pinned[@]}"; do
  expect "rev: v$version" "$file"
done
expect "TurtleTech-ehf/snapper@v$version" docs/orgmode/howto/ci-enforcement.org

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
fi

if [[ "$failed" -eq 0 ]]; then
  printf 'release checks passed for v%s (%s-tag)\n' "$version" "$mode"
fi
exit "$failed"
