#!/usr/bin/env bash
# Install cargo-dist 0.30.3 from a versioned archive after sha256.
# Do not pipe the installer script into sh.
set -euo pipefail

VER=0.30.3
os="${SNAPPER_UNAME_S:-$(uname -s)}"
arch="${SNAPPER_UNAME_M:-$(uname -m)}"

digest() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 "$1" | awk '{print $NF}'
    else
        printf 'no sha256sum, shasum, or openssl on PATH\n' >&2
        exit 1
    fi
}
case "$os" in
Linux)
    case "$arch" in
    x86_64)
        target=x86_64-unknown-linux-gnu
        ext=tar.xz
        sha=214ab0f512b9cad7f26dd4804ecbb73ad898db6beaae48f0ce30baf3b9a763f6
        ;;
    aarch64 | arm64)
        target=aarch64-unknown-linux-gnu
        ext=tar.xz
        sha=721ba4602df82b22bcbe2de615cd2f520b51e50d6faef5f74ddeb6b67cae5116
        ;;
    *)
        printf 'unsupported Linux arch: %s\n' "$arch" >&2
        exit 1
        ;;
    esac
    ;;
Darwin)
    case "$arch" in
    x86_64)
        target=x86_64-apple-darwin
        ext=tar.xz
        sha=81e53353e142ed4bd838be82abc59309cd580e67f8c61eebb4e932ef1982480d
        ;;
    arm64)
        target=aarch64-apple-darwin
        ext=tar.xz
        sha=e5b587c7fe71a89ee3325745984ac412982d6c0e6ec8d4d5072fe579b38765cf
        ;;
    *)
        printf 'unsupported Darwin arch: %s\n' "$arch" >&2
        exit 1
        ;;
    esac
    ;;
MINGW* | MSYS* | CYGWIN*)
    target=x86_64-pc-windows-msvc
    ext=zip
    sha=8fe2d2ce1be4e6e2efe26700490b1765dff203595544bddf3e67378b72c0e228
    ;;
*)
    printf 'unsupported OS: %s\n' "$os" >&2
    exit 1
    ;;
esac

base="cargo-dist-${target}"
url="https://github.com/axodotdev/cargo-dist/releases/download/v${VER}/${base}.${ext}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
if [[ -n "${CARGO_DIST_ARCHIVE:-}" ]]; then
    archive="$CARGO_DIST_ARCHIVE"
    sha="${CARGO_DIST_SHA:-$sha}"
else
    archive="${work}/${base}.${ext}"
    curl --proto '=https' --tlsv1.2 -fsSL -o "$archive" "$url"
fi
got="$(digest "$archive")"
if [[ "$got" != "$sha" ]]; then
    printf 'checksum mismatch for %s\n  expected %s\n  got      %s\n' "${CARGO_DIST_ARCHIVE:-$url}" "$sha" "$got" >&2
    exit 1
fi

dest="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$dest"
if [[ "$ext" == zip ]]; then
    unzip -q -o "$archive" -d "$work"
    bin_name=dist.exe
else
    tar -xJf "$archive" -C "$work"
    bin_name=dist
fi
bin="$(find "$work" -name "$bin_name" -type f -print -quit)"
if [[ -z "$bin" ]]; then
    printf 'cargo-dist archive has no %s\n' "$bin_name" >&2
    exit 1
fi
install -m 0755 "$bin" "${dest}/${bin_name}"
printf 'installed cargo-dist %s to %s\n' "$VER" "$dest"
