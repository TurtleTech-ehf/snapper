# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## Unreleased (main)
#### Bug Fixes
- (**markdown**) keep setext headings (title + `=`/`-` underline) as Structure so multi-sentence titles do not reflow and underlines are not glued onto title text
#### Tests
- Unit and format_text coverage for setext equals/dashes and multi-sentence titles

- - -

## v0.8.0 - 2026-07-08
#### Features
- (**pandoc**) optional `--use-pandoc` path: parse with pandoc (CLI JSON or in-process FFI), classify regions from AST node kinds, reflow only prose (`Para`/`Plain`); structure (headers, code fences, tables, lists, math islands) preserved
- (**pandoc**) `--pandoc-backend auto|ffi|cli` (default auto: prefer warm FFI when available, else CLI); explicit `ffi` fails closed if the library is missing
- (**pandoc**) content-addressed AST cache (`SNAPPER_PANDOC_CACHE` / `SNAPPER_PANDOC_CACHE_DIR`)
- (**pandoc-colink**) opt-in Cargo feature: absorb `libsnapper_pandoc.a` from `native/snapper-pandoc/build-static.sh` (`ghc -staticlib`) into one `snapper` binary — no `SNAPPER_PANDOC_LIB` discovery, no `libHS*` RUNPATH graph; **not** enabled by default (default/cargo-dist remain GHC-free)
#### Bug Fixes
- (**org** / **latex**) keep full headline and sectioning lines as Structure (do not reflow multi-sentence titles; no orphan title lines without stars / mid-brace splits)
#### Documentation
- Native flib README: static archive vs shared dlopen; howto orgmode pandoc backend; UPX is optional packer only (not used in release)
- Formats reference: Org headlines, LaTeX sectioning, and Markdown ATX headings are full Structure lines
#### Tests / CI
- AST fixture tests (JSON on disk) run without pandoc binary; live CLI/FFI parity tests skip cleanly when neither is installed; CI asserts colink without `.a` fails with archive message; default `cargo test` stays green without GHC
- Unit and format_text coverage for multi-sentence Org headlines and LaTeX `\section{...}` titles
#### Notes
- Plain release binaries stay ~16 MiB class; static-colink builds are larger (~64 MiB class on linux) and optional for multi-format in-process use

- - -

## v0.7.9 - 2026-07-07
#### Bug Fixes
- (**markdown**) keep ATX headings as a single Structure line (do not reflow titles after `### 1.` or mid-heading periods; fixes orphan `### N.` lines, snapper-25kc)
#### Tests
- Unit and integration coverage for numbered ATX headings with inline code

- - -

## v0.7.8 - 2026-06-27
#### Features
- (**wasm** / **word**) local `wasm-pack` build of `packages/snapper-wasm` and Word add-in `dist/` for sideload; VS Code extension published **0.7.8** to Marketplace
- (**parser**) `Region::Code` for Org/Markdown/LaTeX/RST source blocks; `[code.<lang>]` comment reflow and optional `--format-code`
- (**sentence**) delimiter-span post-pipeline (quotes, parens, brackets); shared by rules and `--neural`
- (**sentence**) neural path protects inline tokens (links, emphasis, …) before the model, then restores and refines
- (**cli**) multi-file splitter reuse (one build per format/lang/neural/extras key); parallel multi-file always
- (**reflow**) parallel region reflow for large documents (`cli` feature, ≥32 regions)
#### Bug Fixes
- (**org** / **sentence**) trailing `>` on headings; org emphasis not pseudo-headlines; space-safe segment glue after neural cuts
#### Documentation
- Span policy in formats reference; README/pre-commit pin **v0.7.8**; MCP/site/VS Code code-block accuracy
#### Tests / CI
- `sentence_delim_props` matrix + proptest; multi-format and code-comment span tests; CI dogfood `snapper --check` on docs/examples

- - -

## v0.7.7 - 2026-04-11
#### Bug Fixes
- (**config**) honor project config across cli and watch - (5f5179d) - *HaoZeke*

- - -

## v0.7.6 - 2026-04-09
#### Bug Fixes
- (**markdown**) join list-item continuation lines before sentence splitting - (8e45528) - *HaoZeke*

- - -
