#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
wasm-pack build --target web --out-dir packages/snapper-wasm/pkg --no-default-features --features wasm
(cd packages/snapper-wasm && npm install && npm run build)
mkdir -p editors/word/assets
(cd editors/word && npm install && npm run build)
(cd editors/obsidian && npm install && npm run build)
bash scripts/check_obsidian_bundle.sh
echo "Ready: packages/snapper-wasm/pkg + editors/word/dist"
