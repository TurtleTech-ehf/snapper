# snapper-pandoc

In-process pandoc surface for snapper: a small C ABI (`include/snapper_pandoc.h`)
over selected pandoc **readers** (JSON AST out).

## Platform matrix

| OS | How Rust talks to Haskell | Build |
|----|---------------------------|--------|
| **Linux / macOS** | Optional **static colink** (`pandoc-colink`): one `snapper` binary | `./build-static.sh` then `cargo … --features "cli,pandoc,pandoc-colink"` |
| **Windows** | **C FFI** to `snapper_pandoc.dll` (cabal `foreign-library`) | `cabal build snapper_pandoc` then `cargo … --features "cli,pandoc"` |

Same C ABI everywhere (`include/snapper_pandoc.h`). Windows native path is the shared
library + `libloading` (default `pandoc` feature), not PE static-absorb of `ghc -staticlib`.

## Two build products

| Product | How | Used by |
|---------|-----|---------|
| **Static archive** `libsnapper_pandoc.a` | `./build-static.sh` (`ghc -staticlib`) | **`pandoc-colink`** on Unix — absorb into one `snapper` binary |
| **Shared** `.so` / `.dylib` / `.dll` | `cabal build snapper_pandoc` | default `pandoc` feature via `dlopen` / `LoadLibrary` |

Colink is a **Unix build** path (compile archive → link into snapper), not “ship 200 MB of
`libHS*` via RUNPATH.”

## Static archive (Unix colink / one binary)

Requires GHC with the `pandoc` package registered (`ghc-pkg field pandoc id`).

```bash
cd native/snapper-pandoc
./build-static.sh
# writes lib/libsnapper_pandoc.a

export SNAPPER_PANDOC_STATIC_LIB=$PWD/lib/libsnapper_pandoc.a
# Colink product is the CLI bin. Cargo.toml may list cdylib for editor/wasm;
# strip it for this absorb build:
#   sed -i 's/crate-type = \["cdylib", "rlib"\]/crate-type = ["rlib"]/' Cargo.toml
cargo build --release --features "cli,pandoc,pandoc-colink" --bin snapper
```

`build.rs` absorbs the archive into **bins only** (target-OS flags):

| OS | Link notes |
|----|------------|
| **Linux** | `--gc-sections`, `-no-pie`, `--start-group` archive + system libs (`z`,`gmp`,`ffi`,…) |
| **macOS** | `-dead_strip`, `-force_load` archive; libs `z`,`iconv`,`gmp`,`ffi`; Homebrew lib paths |
| **Windows** | Prefer C FFI DLL (below). Static absorb on PE is unsupported in CI. |

### Prerequisites (register pandoc for GHC, Unix colink)

- **Linux**: nix `ghc.withPackages (p: [p.pandoc])`, or `cabal install --lib pandoc …`
- **macOS**: [GHCup](https://www.haskell.org/ghcup/) + `cabal install --lib pandoc pandoc-types aeson`, plus `brew install gmp libffi` if link fails

Optional multi-OS smoke: workflow `colink-os` (Unix colink + Windows DLL FFI). Never blocks PR CI.

## Shared library (C FFI / dlopen / LoadLibrary)

```bash
cd native/snapper-pandoc
cabal build snapper_pandoc
mkdir -p lib
# Unix example:
cp dist-newstyle/build/*/*/*/f/snapper_pandoc/build/snapper_pandoc/libsnapper_pandoc.so* lib/ 2>/dev/null || true
# Windows: copy snapper_pandoc.dll from dist-newstyle into lib/ (and next to snapper.exe)
export SNAPPER_PANDOC_LIB=$PWD/lib/libsnapper_pandoc.so   # or …/snapper_pandoc.dll
cargo build --release --features "cli,pandoc" --bin snapper
# Discovery also checks: SNAPPER_PANDOC_LIB, SNAPPER_PANDOC_LIB_DIR,
# directory of the running executable, native/snapper-pandoc/lib/, dist-newstyle.
```

## C ABI

| Symbol | Role |
|--------|------|
| `snapper_pandoc_parse(format, input, err_out)` | Parse markup → pandoc JSON AST C string |
| `snapper_pandoc_free(ptr)` | Free strings from parse / err_out |
| `snapper_pandoc_hs_ready()` | Optional touch of the Haskell side |

Formats: `markdown`/`gfm`/`commonmark`, `org`, `rst`, `latex`, `html`, `typst`.

The host process must initialize the GHC RTS (`hs_init`) before parse. Rust
bindings in `src/parser/pandoc/ffi.rs` own that lifecycle (process-lifetime argv).

## UPX (optional post-build pack)

[UPX](https://upx.github.io/) is an optional **executable packer**: it compresses a
*finished* binary on disk and decompresses it in memory at start. It is **not** a
compiler tree-shaker and is **not** required for correctness. Default CI and
cargo-dist plain releases **do not** run UPX.

For large **static-colink** binaries only, after a successful colink build:

```bash
# requires `upx` on PATH
./native/snapper-pandoc/pack-upx.sh target/release/snapper
# flags: upx -9 (or -9 --lzma when supported)
# refuses binaries < 20 MiB unless SNAPPER_UPX_FORCE=1
```

Prefer thinning the Haskell build graph (`build-static.sh` / fewer readers) when
you need a structurally smaller binary; packing only shrinks on-disk size.
