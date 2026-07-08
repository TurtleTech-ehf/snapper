# snapper-pandoc

In-process pandoc surface for snapper: a small C ABI (`include/snapper_pandoc.h`)
over selected pandoc **readers** (JSON AST out).

## Two build products

| Product | How | Used by |
|---------|-----|---------|
| **Static archive** `libsnapper_pandoc.a` | `./build-static.sh` (`ghc -staticlib`) | **`pandoc-colink`** — absorbed into one `snapper` binary |
| **Shared** `libsnapper_pandoc.so` | `cabal build snapper_pandoc` | default `pandoc` feature via `dlopen` |

Colink is a **build** path (compile archive → link into snapper), not “ship 200 MB of
`libHS*` via RUNPATH.”

## Static archive (colink / one binary)

Requires GHC with the `pandoc` package registered (`ghc-pkg field pandoc id`).

```bash
cd native/snapper-pandoc
./build-static.sh
# writes lib/libsnapper_pandoc.a

export SNAPPER_PANDOC_LIB_DIR=$PWD/lib   # or SNAPPER_PANDOC_STATIC_LIB=.../libsnapper_pandoc.a
cargo build --release --features "cli,pandoc,pandoc-colink"
```

`build.rs` (feature `pandoc-colink`) links the `.a` with `--whole-archive` and
`--gc-sections`, plus ordinary system libs (`z`, `gmp`, `ffi`, …). It does **not**
inject GHC package-dir rpaths from `ldd`.

Final size is dominated by how much of pandoc the staticlib + GC keep (typically
tens of MB for one executable), not by a tree of separate HS shared objects.

## Shared library (dlopen)

```bash
cd native/snapper-pandoc
cabal build snapper_pandoc
mkdir -p lib
cp dist-newstyle/build/*/*/*/f/snapper_pandoc/build/snapper_pandoc/libsnapper_pandoc.so* lib/
ln -sfn libsnapper_pandoc.so.0.0.0 lib/libsnapper_pandoc.so.0
ln -sfn libsnapper_pandoc.so.0.0.0 lib/libsnapper_pandoc.so
export SNAPPER_PANDOC_LIB=$PWD/lib/libsnapper_pandoc.so
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
