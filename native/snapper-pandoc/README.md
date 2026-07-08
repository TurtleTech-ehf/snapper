# snapper-pandoc

In-process pandoc surface for snapper: a Cabal `foreign-library` that exports
a small C ABI (`include/snapper_pandoc.h`) over the Haskell pandoc library.

## Build

Requires GHC and the `pandoc` Haskell package (matching a recent pandoc 3.x).

```bash
cd native/snapper-pandoc
cabal update
cabal build snapper_pandoc
# Shared object lands under dist-newstyle/build/.../libsnapper_pandoc.so
mkdir -p lib
cp dist-newstyle/build/*/*/*/f/snapper_pandoc/build/snapper_pandoc/libsnapper_pandoc.so* lib/
# SONAME is libsnapper_pandoc.so.0 — colink binaries NEEDED that name.
ln -sfn libsnapper_pandoc.so.0.0.0 lib/libsnapper_pandoc.so.0
ln -sfn libsnapper_pandoc.so.0.0.0 lib/libsnapper_pandoc.so
```

### Dynamic load (default Rust feature `pandoc`)

Set `SNAPPER_PANDOC_LIB` or put the `.so` on the linker path so snapper can
`dlopen` it at runtime.

### Co-link / compile together (feature `pandoc-colink`)

Build the foreign library, then:

```bash
export SNAPPER_PANDOC_LIB_DIR=$PWD/native/snapper-pandoc/lib
cargo build --release --features "cli,pandoc,pandoc-colink"
```

`build.rs` emits the link line (shared foreign-library, not a full static GHC
archive — still one co-built deploy unit via `NEEDED` + `RUNPATH`):

```text
-L$SNAPPER_PANDOC_LIB_DIR
-Wl,--no-as-needed $SNAPPER_PANDOC_LIB_DIR/libsnapper_pandoc.so.0.0.0 -Wl,--as-needed
-lsnapper_pandoc
-Wl,-rpath,$SNAPPER_PANDOC_LIB_DIR
-Wl,-rpath,<each GHC/pandoc package dir from ldd; never host glibc/gmp/zlib>
```

The happy path uses linked symbols (`extern "C" snapper_pandoc_parse`); no
`SNAPPER_PANDOC_LIB` discovery. Prefer `bfd` if mold drops the absolute `.so`
(`RUSTFLAGS='-C link-arg=-fuse-ld=bfd'`).

## C ABI

| Symbol | Role |
|--------|------|
| `snapper_pandoc_parse(format, input, err_out)` | Parse markup → pandoc JSON AST C string |
| `snapper_pandoc_free(ptr)` | Free strings from parse / err_out |
| `snapper_pandoc_hs_ready()` | Optional touch of the Haskell side |

The host process must initialize the GHC RTS (`hs_init` / `hs_exit`) before
calling parse. The Rust bindings in `src/parser/pandoc/ffi.rs` own that lifecycle.
