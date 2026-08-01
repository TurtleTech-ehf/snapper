#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
public_sources=(
  "$repo_root/readme_src.org"
  "$repo_root/README.md"
  "$repo_root/npm/README.md"
  "$repo_root/docs/orgmode/tutorials/quickstart.org"
  "$repo_root/docs/orgmode/howto/editor-integration.org"
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
  "Install the snapper plugin from Community Plugins"
  "Install from Community Plugins"
  "Install the snapper add-in from AppSource"
  "In Word: Insert > Get Add-ins > search \"Snapper\""
  "Download =main.js= and =manifest.json= from"
  "Download the add-in ZIP from"
)

failed=0

for pattern in "${required_patterns[@]}"; do
  if ! grep -Fq "$pattern" "${public_sources[@]}"; then
    printf 'public docs are missing required content: %s\n' "$pattern" >&2
    failed=1
  fi
done

for pattern in "${unsupported_patterns[@]}"; do
  if grep -Fiq "$pattern" "${public_sources[@]}"; then
    printf 'public docs claim an unavailable distribution: %s\n' "$pattern" >&2
    failed=1
  fi
done

exit "$failed"
