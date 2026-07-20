#!/usr/bin/env bash
# Optional post-build UPX pack for a *finished* static-colink snapper binary.
# Not a tree-shaker; not required for correctness; not part of default CI.
#
# Usage:
#   ./native/snapper-pandoc/pack-upx.sh [path/to/snapper]
#   SNAPPER_UPX_FORCE=1 ./native/snapper-pandoc/pack-upx.sh ./target/release/snapper
#
# Default flags: upx -9 (best compression for release-sized colink binaries).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${1:-$ROOT/target/release/snapper}"
FORCE="${SNAPPER_UPX_FORCE:-0}"

if [[ ! -f "$BIN" ]]; then
  echo "pack-upx: binary not found: $BIN" >&2
  echo "Build static-colink first:" >&2
  echo "  cd native/snapper-pandoc && ./build-static.sh" >&2
  echo "  SNAPPER_PANDOC_STATIC_LIB=... cargo build --release --features 'cli,pandoc,pandoc-colink' --bin snapper" >&2
  exit 1
fi

if ! command -v upx >/dev/null 2>&1; then
  echo "pack-upx: 'upx' not on PATH (install from https://upx.github.io/ or distro package)" >&2
  exit 1
fi

# Refuse to pack a tiny plain binary by default (colink is ~tens of MiB).
bytes=$(stat -c%s "$BIN" 2>/dev/null || stat -f%z "$BIN")
min_bytes=$((20 * 1024 * 1024))
if [[ "$FORCE" != "1" && "$bytes" -lt "$min_bytes" ]]; then
  echo "pack-upx: refusing to pack small binary (${bytes} bytes < ${min_bytes})." >&2
  echo "This script targets static-colink (~64MiB). Set SNAPPER_UPX_FORCE=1 to override." >&2
  exit 1
fi

# Skip if already UPX-packed
if upx -t "$BIN" >/dev/null 2>&1; then
  echo "pack-upx: already packed: $BIN ($(stat -c%s "$BIN" 2>/dev/null || stat -f%z "$BIN") bytes)"
  exit 0
fi

before=$bytes
echo "pack-upx: before=${before} bytes  bin=${BIN}"
# -9: best ratio; --lzma if available for extra shrink (fallback without).
if upx -9 --lzma "$BIN" 2>/dev/null; then
  :
else
  upx -9 "$BIN"
fi
after=$(stat -c%s "$BIN" 2>/dev/null || stat -f%z "$BIN")
echo "pack-upx: after=${after} bytes  ratio=$(awk -v a="$after" -v b="$before" 'BEGIN{printf "%.1f%%", 100*a/b}')"
upx -t "$BIN"
echo "pack-upx: ok"
