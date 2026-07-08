#!/usr/bin/env bash
# Build-time absorbable archive for pandoc-colink (one .a, not shared+rpath).
# Requires: ghc with pandoc package (e.g. nix ghc-with-packages).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="${SNAPPER_PANDOC_LIB_DIR:-$ROOT/lib}"
BUILD_DIR="$ROOT/static-build"
mkdir -p "$OUT_DIR" "$BUILD_DIR"

if ! command -v ghc >/dev/null 2>&1; then
  echo "build-static: ghc not on PATH" >&2
  exit 1
fi

PID="$(ghc-pkg field pandoc id --simple-output 2>/dev/null | head -1 || true)"
if [[ -z "${PID}" ]]; then
  echo "build-static: pandoc not registered in ghc-pkg (install pandoc Haskell package)" >&2
  exit 1
fi

echo "build-static: ghc $(ghc --numeric-version) pandoc unit $PID"
# -staticlib folds our module + reachable package objects into one archive.
# Final executable size is cut at link with --gc-sections (see build.rs).
# -split-sections helps the final --gc-sections link drop dead object sections.
ghc -O2 -fPIC -split-sections -staticlib \
  -package-id "$PID" \
  -package aeson -package bytestring -package text \
  -i"$ROOT/src" \
  -outputdir "$BUILD_DIR" \
  -o "$BUILD_DIR/libsnapper_pandoc.a" \
  "$ROOT/src/SnapperPandoc.hs"

cp -f "$BUILD_DIR/libsnapper_pandoc.a" "$OUT_DIR/libsnapper_pandoc.a"
# Keep shared build optional for dlopen path; do not require it for colink.
ls -lh "$OUT_DIR/libsnapper_pandoc.a"
echo "build-static: wrote $OUT_DIR/libsnapper_pandoc.a"
