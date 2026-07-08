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

# Windows: MinGW ld from chocolatey often rejects GHC's ar members with
# "…(SnapperPandoc.o) in archive is not an object". Re-extract and repack
# with the same MinGW ar that cargo will drive, dropping non-COFF members.
case "$uname_s" in
  MINGW*|MSYS*|CYGWIN*)
    REPACK="$BUILD_DIR/repack-objs"
    rm -rf "$REPACK"
    mkdir -p "$REPACK"
    (
      cd "$REPACK"
      # Prefer the ar that matches the rustc windows-gnu linker.
      AR_BIN="$(command -v x86_64-w64-mingw32-ar 2>/dev/null || command -v ar)"
      OD_BIN="$(command -v x86_64-w64-mingw32-objdump 2>/dev/null || command -v objdump || true)"
      echo "build-static: windows repack with AR=$AR_BIN OD=${OD_BIN:-none}"
      # Extract with the same mingw ar cargo's ld will use.
      "$AR_BIN" x "$BUILD_DIR/libsnapper_pandoc.a"
      ls -la | head -40 || true
      # Flatten one level of nested archives (ghc -staticlib sometimes embeds
      # an ar member named *.o that is itself an archive — MinGW ld then says
      # "member … is not an object").
      for f in ./*; do
        [[ -f "$f" ]] || continue
        if head -c 8 "$f" 2>/dev/null | grep -q '!<arch>'; then
          echo "build-static: flatten nested archive $(basename "$f")"
          sub="$REPACK/nested-$(basename "$f")"
          mkdir -p "$sub"
          (
            cd "$sub"
            "$AR_BIN" x "$f"
          )
          rm -f "$f"
          # Hoist nested members with unique names.
          for n in "$sub"/*; do
            [[ -f "$n" ]] || continue
            mv -f "$n" "./nested_$(basename "$f")__$(basename "$n")"
          done
          rmdir "$sub" 2>/dev/null || true
        fi
      done
      # Diagnose SnapperPandoc* if present.
      for f in ./SnapperPandoc* ./nested_*SnapperPandoc*; do
        [[ -f "$f" ]] || continue
        echo "build-static: diagnose $f"
        xxd "$f" 2>/dev/null | head -2 || od -A x -t x1z -N 32 "$f" 2>/dev/null || true
        [[ -n "$OD_BIN" ]] && "$OD_BIN" -f "$f" 2>&1 | head -8 || true
      done
      objs=()
      for f in ./*; do
        [[ -f "$f" ]] || continue
        base="$(basename "$f")"
        case "$base" in
          *.hi|*.hie|*.dyn_hi|*.p_hi) echo "build-static: drop $base"; continue ;;
        esac
        # Still a nested archive? drop (should have been flattened).
        if head -c 8 "$f" 2>/dev/null | grep -q '!<arch>'; then
          echo "build-static: drop still-nested $base"
          continue
        fi
        if [[ -n "$OD_BIN" ]]; then
          if "$OD_BIN" -f "$f" >/dev/null 2>&1; then
            objs+=("$f")
          else
            echo "build-static: drop non-object $base"
            "$OD_BIN" -f "$f" 2>&1 | head -3 || true
          fi
        else
          objs+=("$f")
        fi
      done
      if [[ ${#objs[@]} -eq 0 ]]; then
        echo "build-static: no COFF objects after extract — keeping ghc archive" >&2
      else
        rm -f "$OUT_DIR/libsnapper_pandoc.a"
        "$AR_BIN" rcs "$OUT_DIR/libsnapper_pandoc.a" "${objs[@]}"
        echo "build-static: repacked ${#objs[@]} objects into $OUT_DIR/libsnapper_pandoc.a"
        # Verify every member is an object for the same ld.
        if [[ -n "$OD_BIN" ]]; then
          bad=0
          while IFS= read -r m; do
            [[ -z "$m" ]] && continue
            "$AR_BIN" p "$OUT_DIR/libsnapper_pandoc.a" "$m" >"$REPACK/_member.bin" 2>/dev/null || continue
            if ! "$OD_BIN" -f "$REPACK/_member.bin" >/dev/null 2>&1; then
              echo "build-static: VERIFY FAIL member $m" >&2
              bad=1
            fi
          done < <("$AR_BIN" t "$OUT_DIR/libsnapper_pandoc.a" 2>/dev/null | head -50)
          echo "build-static: member verify bad=$bad (first 50 checked)"
        fi
      fi
    )
    ;;
esac

ls -lh "$OUT_DIR/libsnapper_pandoc.a"
echo "build-static: wrote $OUT_DIR/libsnapper_pandoc.a"
