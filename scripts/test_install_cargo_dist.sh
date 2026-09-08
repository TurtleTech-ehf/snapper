#!/usr/bin/env bash
# Regression tests for scripts/install_cargo_dist.sh.
# Reproduces the v0.11.0 Release failures:
#   aarch64-apple-darwin: sha256sum: command not found
#   windows-msvc: install cannot stat nested dist.exe (zip is flat)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
installer="$root/scripts/install_cargo_dist.sh"
failed=0
fail() { printf 'FAIL: %s\n' "$1" >&2; failed=1; }
pass() { printf 'ok: %s\n' "$1"; }

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# --- hasher without sha256sum (macOS ARM runners) ---
hash_bin="$scratch/hashpath"
mkdir -p "$hash_bin"
# Keep the tools the installer needs; omit sha256sum.
for cmd in bash curl mktemp mkdir install tar unzip awk printf uname tr cat chmod rm ls find openssl xz xzcat; do
    if p=$(command -v "$cmd"); then
        ln -s "$p" "$hash_bin/$cmd"
    fi
done
if p=$(command -v shasum); then
    ln -s "$p" "$hash_bin/shasum"
fi
# A tiny local archive so this test does not depend on GitHub.
payload="$scratch/payload"
mkdir -p "$payload/cargo-dist-x86_64-unknown-linux-gnu"
printf '#!/bin/sh\necho dist-ok\n' >"$payload/cargo-dist-x86_64-unknown-linux-gnu/dist"
chmod +x "$payload/cargo-dist-x86_64-unknown-linux-gnu/dist"
archive="$scratch/cargo-dist-x86_64-unknown-linux-gnu.tar.xz"
tar -C "$payload" -cJf "$archive" cargo-dist-x86_64-unknown-linux-gnu
want_sha="$(sha256sum "$archive" | awk '{print $1}')"

home="$scratch/home-hash"
mkdir -p "$home"
set +e
PATH="$hash_bin" HOME="$home" CARGO_HOME="$home/.cargo" \
    SNAPPER_UNAME_S=Linux SNAPPER_UNAME_M=x86_64 \
    CARGO_DIST_ARCHIVE="$archive" CARGO_DIST_SHA="$want_sha" \
    bash "$installer" >"$scratch/hash.out" 2>"$scratch/hash.err"
rc=$?
set -e
if [[ "$rc" -ne 0 ]]; then
    fail "installer exits $rc when sha256sum is absent: $(tr '\n' ' ' <"$scratch/hash.err")"
elif [[ ! -x "$home/.cargo/bin/dist" ]]; then
    fail "hasher path did not install dist"
else
    pass "installer hashes without sha256sum"
fi

# --- flat Windows zip (real cargo-dist 0.30.3 layout) ---
win_dir="$scratch/win"
mkdir -p "$win_dir"
printf 'MZ-fake\n' >"$win_dir/dist.exe"
printf 'license\n' >"$win_dir/LICENSE-MIT"
win_zip="$scratch/cargo-dist-x86_64-pc-windows-msvc.zip"
python3 - "$win_dir" "$win_zip" <<'PY'
import sys, zipfile
from pathlib import Path
src, dest = Path(sys.argv[1]), Path(sys.argv[2])
with zipfile.ZipFile(dest, "w") as zf:
    zf.write(src / "dist.exe", "dist.exe")
    zf.write(src / "LICENSE-MIT", "LICENSE-MIT")
PY
win_sha="$(sha256sum "$win_zip" | awk '{print $1}')"
home_win="$scratch/home-win"
mkdir -p "$home_win"
set +e
SNAPPER_UNAME_S=MINGW64_NT-10.0 SNAPPER_UNAME_M=x86_64 \
    CARGO_DIST_ARCHIVE="$win_zip" CARGO_DIST_SHA="$win_sha" \
    HOME="$home_win" CARGO_HOME="$home_win/.cargo" \
    bash "$installer" >"$scratch/win.out" 2>"$scratch/win.err"
rc=$?
set -e
if [[ "$rc" -ne 0 ]]; then
    fail "flat zip install exits $rc: $(tr '\n' ' ' <"$scratch/win.err")"
elif [[ ! -f "$home_win/.cargo/bin/dist.exe" ]]; then
    fail "flat zip did not install dist.exe to CARGO_HOME/bin (err=$(tr '\n' ' ' <"$scratch/win.err"))"
else
    pass "installer finds dist.exe at zip root"
fi

if [[ "$failed" -ne 0 ]]; then
    exit 1
fi
printf 'all install_cargo_dist tests passed\n'
