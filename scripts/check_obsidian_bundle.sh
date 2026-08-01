#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bundle="${1:-$repo_root/editors/obsidian/main.js}"

if [[ ! -s "$bundle" ]]; then
  printf 'Obsidian bundle is missing or empty: %s\n' "$bundle" >&2
  exit 1
fi

if grep -Fq 'import_meta = {}' "$bundle"; then
  printf 'Obsidian CommonJS bundle contains an unusable import.meta substitute\n' >&2
  exit 1
fi

if ! grep -Fq 'AGFzbQE' "$bundle"; then
  printf 'Obsidian bundle does not contain the WebAssembly module\n' >&2
  exit 1
fi
