#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
public_sources=(
  "$repo_root/readme_src.org"
  "$repo_root/README.md"
  "$repo_root/npm/README.md"
  "$repo_root/docs/orgmode/tutorials/quickstart.org"
  "$repo_root/docs/orgmode/howto/editor-integration.org"
  "$repo_root/docs/orgmode/howto/mcp-integration.org"
  "$repo_root/editors/obsidian/README.md"
  "$repo_root/editors/word/README.md"
)

required_patterns=(
  "cargo install snapper-fmt --features mcp"
  "snapper mcp"
  "marketplace.visualstudio.com/items?itemName=TurtleTech.snapper"
  "Development preview; not listed in Community Plugins"
  "Development preview; not published in AppSource"
)

unsupported_patterns=(
  "npx @turtletech/snapper-mcp"
  "@turtletech/snapper-mcp"
  '"command": "npx"'
  "Install the snapper plugin from Community Plugins"
  "Install from Community Plugins"
  "Install the snapper add-in from AppSource"
  "In Word: Insert > Get Add-ins > search \"Snapper\""
  "Download =main.js= and =manifest.json= from"
  "Download the add-in ZIP from"
)

failed=0

if ! grep -Fq '"private": true' "$repo_root/npm/package.json"; then
  printf 'unpublished npm wrapper must be private\n' >&2
  failed=1
fi

if grep -Eq '\|\|[[:space:]]+true' "$repo_root/scripts/build_wasm_editors.sh"; then
  printf 'editor build helper must propagate every build failure\n' >&2
  failed=1
fi

if grep -Eq 'npm[[:space:]]+install' "$repo_root/scripts/build_wasm_editors.sh"; then
  printf 'editor build helper must use locked npm installs\n' >&2
  failed=1
fi

for pattern in "${required_patterns[@]}"; do
  if ! grep -Fq "$pattern" "${public_sources[@]}"; then
    printf 'public docs are missing required content: %s\n' "$pattern" >&2
    failed=1
  fi
done

obsidian_sources=(
  "$repo_root/readme_src.org"
  "$repo_root/README.md"
  "$repo_root/docs/orgmode/tutorials/quickstart.org"
  "$repo_root/docs/orgmode/howto/editor-integration.org"
  "$repo_root/editors/obsidian/README.md"
)
for file in "${obsidian_sources[@]}"; do
  if ! grep -Fq "Development preview; not listed in Community Plugins" "$file"; then
    printf 'Obsidian preview status missing from %s\n' "$file" >&2
    failed=1
  fi
done

word_preview_sources=(
  "$repo_root/readme_src.org"
  "$repo_root/README.md"
  "$repo_root/docs/orgmode/tutorials/quickstart.org"
  "$repo_root/docs/orgmode/howto/editor-integration.org"
  "$repo_root/editors/word/README.md"
)
for file in "${word_preview_sources[@]}"; do
  if ! grep -Fq "Development preview; not published in AppSource" "$file"; then
    printf 'Word preview status missing from %s\n' "$file" >&2
    failed=1
  fi
done

for pattern in "${unsupported_patterns[@]}"; do
  if grep -Fiq "$pattern" "${public_sources[@]}"; then
    printf 'public docs claim an unavailable distribution: %s\n' "$pattern" >&2
    failed=1
  fi
done

release_version="$(sed -n '/^\[package\]$/,/^\[/s/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
word_versions=(
  "$repo_root/editors/word/package.json"
  "$repo_root/editors/word/manifest.xml"
  "$repo_root/editors/word/README.md"
  "$repo_root/editors/word/src/taskpane/taskpane.html"
)

for file in "${word_versions[@]}"; do
  if ! grep -Fq "$release_version" "$file"; then
    printf 'Word integration version does not match snapper %s: %s\n' \
      "$release_version" "$file" >&2
    failed=1
  fi
done

exit "$failed"
