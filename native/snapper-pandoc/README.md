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
```

Install the `.so` on the linker path (or set `SNAPPER_PANDOC_LIB`) so the Rust
`pandoc-ffi` feature can `dlopen` it.

## C ABI

| Symbol | Role |
|--------|------|
| `snapper_pandoc_parse(format, input, err_out)` | Parse markup → pandoc JSON AST C string |
| `snapper_pandoc_free(ptr)` | Free strings from parse / err_out |
| `snapper_pandoc_hs_ready()` | Optional touch of the Haskell side |

The host process must initialize the GHC RTS (`hs_init` / `hs_exit`) before
calling parse. The Rust bindings in `src/parser/pandoc/ffi.rs` own that lifecycle.
