#!/usr/bin/env bash
# Install cargo-dist 0.30.3 from a versioned archive after sha256.
# Do not pipe the installer script into sh.
set -euo pipefail

VER=0.30.3
os="$(uname -s)"
arch="$(uname -m)"
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
archive="${work}/${base}.${ext}"
curl --proto '=https' --tlsv1.2 -fsSL -o "$archive" "$url"
got="$(sha256sum "$archive" | awk '{print $1}')"
if [[ "$got" != "$sha" ]]; then
    printf 'checksum mismatch for %s\n  expected %s\n  got      %s\n' "$url" "$sha" "$got" >&2
    exit 1
fi

dest="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$dest"
if [[ "$ext" == zip ]]; then
    unzip -q -o "$archive" -d "$work"
    install -m 0755 "${work}/${base}/dist.exe" "${dest}/dist.exe"
else
    tar -xJf "$archive" -C "$work"
    install -m 0755 "${work}/${base}/dist" "${dest}/dist"
fi
printf 'installed cargo-dist %s to %s\n' "$VER" "$dest"
