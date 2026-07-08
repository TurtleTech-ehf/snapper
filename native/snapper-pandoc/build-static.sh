#!/usr/bin/env bash
# Build-time absorbable archive for pandoc-colink (ghc -staticlib).
# Linux / macOS / Windows(MSYS2 or GHCup bash). Requires ghc + registered pandoc.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="${SNAPPER_PANDOC_LIB_DIR:-$ROOT/lib}"
BUILD_DIR="$ROOT/static-build"
mkdir -p "$OUT_DIR" "$BUILD_DIR"

if ! command -v ghc >/dev/null 2>&1; then
  echo "build-static: ghc not on PATH" >&2
  exit 1
fi

# Resolve pandoc unit id: explicit env, ghc-pkg, or GHC_ENVIRONMENT package-env file.
PID="${SNAPPER_PANDOC_PACKAGE_ID:-}"
if [[ -z "${PID}" ]]; then
  PID="$(ghc-pkg field pandoc id --simple-output 2>/dev/null | head -1 | tr -d '\r' || true)"
fi
if [[ -z "${PID}" && -n "${GHC_ENVIRONMENT:-}" && -f "${GHC_ENVIRONMENT}" ]]; then
  PID="$(grep -E '^package-id pandoc-' "$GHC_ENVIRONMENT" | head -1 | awk '{print $2}' | tr -d '\r' || true)"
fi
if [[ -z "${PID}" ]]; then
  # Walk upward for .ghc.environment.* (cabal install --lib --package-env=.)
  search="$ROOT"
  for _ in 1 2 3 4; do
    envf="$(ls -1 "$search"/.ghc.environment.* 2>/dev/null | head -1 || true)"
    if [[ -n "$envf" ]]; then
      PID="$(grep -E '^package-id pandoc-' "$envf" | head -1 | awk '{print $2}' | tr -d '\r' || true)"
      if [[ -n "$PID" ]]; then
        export GHC_ENVIRONMENT="$envf"
        break
      fi
    fi
    search="$(dirname "$search")"
  done
fi
if [[ -z "${PID}" ]]; then
  echo "build-static: pandoc not registered in ghc-pkg / package-env" >&2
  echo "  Install a GHC that includes the pandoc package, or:" >&2
  echo "    cabal install --lib --package-env=. pandoc pandoc-types aeson" >&2
  echo "  Or set SNAPPER_PANDOC_PACKAGE_ID / GHC_ENVIRONMENT." >&2
  exit 1
fi

uname_s="$(uname -s 2>/dev/null || echo unknown)"
echo "build-static: os=${uname_s} ghc=$(ghc --numeric-version) pandoc unit ${PID}"
if [[ -n "${GHC_ENVIRONMENT:-}" ]]; then
  echo "build-static: GHC_ENVIRONMENT=${GHC_ENVIRONMENT}"
fi

# Platform-tuned GHC flags.
GHC_FLAGS=(-O2 -staticlib)
case "$uname_s" in
  Darwin)
    # PIC for dylib-friendly objects; no -split-sections required on modern GHC.
    GHC_FLAGS+=(-fPIC)
    ;;
  MINGW*|MSYS*|CYGWIN*)
    GHC_FLAGS+=(-fPIC)
    ;;
  *)
    GHC_FLAGS+=(-fPIC -split-sections)
    ;;
esac

# Prefer package-env when present so transitive deps resolve like cabal install --lib.
GHC_ENV_ARGS=()
if [[ -n "${GHC_ENVIRONMENT:-}" && -f "${GHC_ENVIRONMENT}" ]]; then
  GHC_ENV_ARGS+=(-package-env "$GHC_ENVIRONMENT")
fi

ghc "${GHC_FLAGS[@]}" \
  "${GHC_ENV_ARGS[@]}" \
  -package-id "$PID" \
  -package aeson -package bytestring -package text \
  -i"$ROOT/src" \
  -outputdir "$BUILD_DIR" \
  -o "$BUILD_DIR/libsnapper_pandoc.a" \
  "$ROOT/src/SnapperPandoc.hs"

cp -f "$BUILD_DIR/libsnapper_pandoc.a" "$OUT_DIR/libsnapper_pandoc.a"
ls -lh "$OUT_DIR/libsnapper_pandoc.a"
echo "build-static: wrote $OUT_DIR/libsnapper_pandoc.a"
