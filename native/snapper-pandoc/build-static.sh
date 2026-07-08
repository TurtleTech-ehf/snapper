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

# Resolve absolute package-env path (relative GHC_ENVIRONMENT breaks after cd).
resolve_env_file() {
  local cand="${1:-}"
  if [[ -n "$cand" && -f "$cand" ]]; then
    # Prefer realpath when available; else dirname/basename.
    if command -v realpath >/dev/null 2>&1; then
      realpath "$cand"
    else
      (cd "$(dirname "$cand")" && echo "$(pwd)/$(basename "$cand")")
    fi
    return 0
  fi
  # Relative name against process cwd (repo root in GHA).
  if [[ -n "$cand" && -f "$PWD/$cand" ]]; then
    echo "$PWD/$(basename "$cand")"
    return 0
  fi
  # Prefer explicit snapper-colink env name.
  if [[ -f "$PWD/.ghc.environment.snapper-colink" ]]; then
    echo "$PWD/.ghc.environment.snapper-colink"
    return 0
  fi
  local search
  for search in "$PWD" "$ROOT" "$(dirname "$ROOT")" "$(dirname "$(dirname "$ROOT")")"; do
    # shellcheck disable=SC2012
    local envf
    envf="$(ls -1 "$search"/.ghc.environment.* 2>/dev/null | head -1 || true)"
    if [[ -n "$envf" && -f "$envf" ]]; then
      echo "$envf"
      return 0
    fi
  done
  return 1
}

ENV_FILE=""
if ENV_FILE="$(resolve_env_file "${GHC_ENVIRONMENT:-}")"; then
  export GHC_ENVIRONMENT="$ENV_FILE"
  echo "build-static: resolved GHC_ENVIRONMENT=$GHC_ENVIRONMENT"
else
  ENV_FILE=""
  unset GHC_ENVIRONMENT || true
fi

# How to name pandoc for ghc: -package-id UNIT or -package pandoc (package-env).
PKG_ARGS=()
PID="${SNAPPER_PANDOC_PACKAGE_ID:-}"
if [[ -z "${PID}" ]]; then
  PID="$(ghc-pkg field pandoc id --simple-output 2>/dev/null | head -1 | tr -d '\r' || true)"
fi
if [[ -z "${PID}" && -n "${ENV_FILE}" ]]; then
  # Cabal env may list "package-id pandoc-3.10-..." or only package-db roots.
  PID="$(grep -E '^package-id[[:space:]]+pandoc-' "$ENV_FILE" | head -1 | awk '{print $2}' | tr -d '\r' || true)"
fi
if [[ -n "${PID}" ]]; then
  PKG_ARGS+=(-package-id "$PID")
  echo "build-static: using -package-id $PID"
elif [[ -n "${ENV_FILE}" ]] && ghc -package-env "$ENV_FILE" -package pandoc -e 'putStrLn "pandoc-ok"' >/dev/null 2>&1; then
  # package-env with package-db lines and no per-unit package-id entries (common
  # with cabal install --lib --package-env=.).
  PKG_ARGS+=(-package pandoc)
  echo "build-static: using -package pandoc via package-env"
else
  echo "build-static: pandoc not registered in ghc-pkg / package-env" >&2
  echo "  Install a GHC that includes the pandoc package, or:" >&2
  echo "    cabal install --lib --package-env=. pandoc pandoc-types aeson" >&2
  echo "  Or set SNAPPER_PANDOC_PACKAGE_ID / GHC_ENVIRONMENT (absolute path)." >&2
  if [[ -n "${ENV_FILE}" ]]; then
    echo "  Env file was: $ENV_FILE" >&2
    echo "  --- head ---" >&2
    head -40 "$ENV_FILE" >&2 || true
  fi
  exit 1
fi

uname_s="$(uname -s 2>/dev/null || echo unknown)"
echo "build-static: os=${uname_s} ghc=$(ghc --numeric-version)"
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
  "${PKG_ARGS[@]}" \
  -package aeson -package bytestring -package text \
  -i"$ROOT/src" \
  -outputdir "$BUILD_DIR" \
  -o "$BUILD_DIR/libsnapper_pandoc.a" \
  "$ROOT/src/SnapperPandoc.hs"

cp -f "$BUILD_DIR/libsnapper_pandoc.a" "$OUT_DIR/libsnapper_pandoc.a"
ls -lh "$OUT_DIR/libsnapper_pandoc.a"
echo "build-static: wrote $OUT_DIR/libsnapper_pandoc.a"
