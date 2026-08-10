# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## Unreleased (main)

#### Features
- (**latex**) `[latex].verbatim_envs` / `structure_envs` / `verbatim_commands` in `.snapperrc.toml` add names to the built-in lists (minted/lstlisting/verbatim, `NON_PROSE_ENVS`, `\\verb`/`\\lstinline`); missing keys keep today's defaults. Extra command names are tokenized like `\\verb` before split. No regex `other:` key
- (**reflow**) `--clause-breaks` / `clause_breaks` with `max_width = 0` inserts a newline after every independent-clause mark (`,`, `;`, `:`, em dash, `--`) that is already followed by whitespace; a one-clause sentence stays one line; `max_width > 0` keeps wrap-prefer-clause. Tokens such as `1,000`, URLs, `--flags`, and unspaced dashes never split. `--check` uses the same mode
- (**cli** / **check**) `--check --output-format json|sarif` emits 1-indexed `fused` / `wrap` / `long` diagnostics with excerpts; `long` is advisory unless `--strict-long`; stdin `--check` honors the same JSON/SARIF and exit codes; SARIF URIs are repo-relative (or `file:` plus `invocations[0].workingDirectory`)
- (**mcp**) `mcp` is a default feature, so published `snapper` / `snapper-fmt` binaries include the stdio server; agents should call it (or the CLI) instead of applying sembr/skills by hand
- (**mcp**) `format_text` accepts `clause_breaks`, `range` (`start`/`end`, 1-indexed inclusive), and `max_width` (default 0)
- (**mcp**) `check_formatting` returns `would_reformat` identical to CLI `--check`, plus the same line diagnostics
- (**safety**) native parsers record source byte ranges and splice reflowed prose into the original document; Structure/Code/Blank are input slices
- (**safety**) `format_text` runs to a byte fixpoint (cap 4, including A/B cycles) or returns the original; a format-local oracle (region-kind + slice tree; Markdown HTML plus a code-byte check) mismatch also returns the original
- (**safety**) `format_bytes` refuses invalid UTF-8 with `InvalidUtf8Error`; `--use-pandoc` refuses (`PandocCannotSplice`) because the AST has no source offsets
- (**safety**) property tests and a `cargo fuzz` target (fixture corpus) drive `format_text` per format with the backstops off
- (**code-block**) comment reflow copies non-comment lines as original slices; only comment spans are rewritten
#### Bug Fixes
- (**latex**) render oracle and `--check` payloads honor `[latex]` extras, so a configured `\\Verb!%!` is not a comment and production backstops no longer revert the file
- (**latex**) tokenize `\\verb` / `\\lstinline` (optional `[...]`) before split so inner `%` is not a comment and inner `.!?` do not split; unmatched `\\verb` runs to EOL; unlisted `\\begin` / `\\end` are region bounds (optional `[...]` stays on the begin token); mid-line `\\begin{equation}` leaves leading words as prose; nested same-name envs (including verbatim/lstlisting) close on matching depth, so `\\end{python}` inside lstlisting does not steal the closer; listing body scans raw `\\begin`/`\\end` (no `%` stop, no `\\verb` skip) so `print(1) % \\end{lstlisting}` and `print(\"%\")` still close
- (**check**) fused and long run on the parser prose payload, so `1. Hello world.` and `See Fig. 1. % TODO cite` are not false fused
- (**reflow**) splice keeps the trailing space before a mid-line TeX `%` comment so that line stays one source line
- (**reflow**) `--max-width` keeps links, images, inline code, autolinks, `$math$`, and Org `[[...]]` on one line (overlong token may sit alone; no hyphen splits); wrap-created lines that the format grammar would read as a new block (fences, thematic breaks, `[ref]:`, HTML, `>foo`, LaTeX `%`/`\begin`/`\section`, RST `..`, Org `|`, lists) are backslash-escaped in Markdown or skip-cut (loop) elsewhere; `<https://...>` / `<user@host>` are autolinks, not HTML openers, so wrap does not inject `\\<`; skip-cut wins when a `\\` would corrupt an inline token; wrap measures hanging indent and tests interrupt after it; no NBSP; first line of a list item stays a list; second pass does not accumulate backslashes; the render oracle treats wrap-created Markdown escapes (`\-`, `1\\.`) as the source words so splice does not return the original
- (**reflow**) list continuation sentences hang at the marker width so Org rejoins the item on reparse; Markdown quotes repeat the `>` prefix (`> One.` / `> Two.`, nested `> >`)
- (**latex**) a trailing `%` eats the newline (TeX nospace join), so `foo%\\nbar` is no longer emitted as `foo% bar` (which comments out `bar`); mid-line `%` comments leave prose
- (**sentence**) `w.r.t.` is a multi-word abbreviation; `\\(...\\)` and `$$...$$` stay atomic like `$...$`
- (**cli**) unknown source extensions (`.rs`, `.py`, no extension) are refused unless `--format` is explicit; `.txt` remains plaintext
- (**org** / **sentence**) verbatim and inline-code spans pair to the first closer that satisfies Org's border and post rules, so an `=` or `~` inside the span no longer orphans the real closer onto the next line
- (**sentence**) CommonMark `*`/`**` and GFM `~~` pair by flanking rules, so `**the end. Still bold**` stays one sentence while `**complex**. Equity` may split after the closer
- (**markdown**) `>` is kept on each reflowed content line (including `max_width` wraps; prefix counts toward width); two-space and backslash hard breaks are not joined with a space; a quote hard break does not leave a stray `>` on the next non-quote line; multiline `<!-- ... -->` is structure (`<!-- snapper:off -->` still works)
#### Documentation
- README distinguishes snapper from admk/sembr and sembr/skills; crate keywords include `sembr` and `markdown`
- (**markdown** / **sentence**) inline code delimited by two or more backticks can contain a shorter backtick run
- (**code-block**) a block-comment closer inside a string no longer ends the comment; quote-sequence closers (`"""`) still match naively so Python docstrings reflow
- (**sentence**) abbreviation merge does not invent a space before LaTeX `~` (so `Eq.~\ref{}` stays attached once org `~code~` pairing no longer swallows that tilde)
- (**sentence**) restore wrapped placeholders from the outside in, so a markdown-link regex match that contains a paired-span token cannot leak `\x00PHn\x00` into output

- - -

## v0.9.1 - 2026-08-04
#### Bug Fixes
- (**reflow**) clause breaks only engage when a sentence exceeds `max_width` and only at whitespace after the punctuation; v0.9.0 split at every clause mark, including inside tokens (`1,000`, `10:30`, URLs, `--flags`, unspaced em dashes), which inserted spaces into rendered output
- (**cli**) `--color auto` honors the `NO_COLOR` environment variable
- (**neural**) nnsplit model loads are serialized process-wide; concurrent first-use construction could read a partially downloaded model
#### Notes
- v0.9.1 supersedes v0.9.0 on every channel; do not use v0.9.0 with `--clause-breaks`
- `RELEASING.md` documents the release channels and recovery rules; `scripts/check_release_ready.sh` gates version parity, tag hygiene, and the conda sha256

- - -

## v0.9.0 - 2026-08-04
#### Features
- (**cli** / **diff**) global `--color auto|always|never` for top-level `--diff`, `sdiff`, and `git-diff`; shared ANSI styling for headers, hunks, additions, and removals (closes #6)
- (**reflow**) `--clause-breaks` / `clause_breaks` config: prefer soft breaks after independent-clause punctuation (comma, semicolon, colon, em dash) under `max_width` (closes #7)
#### Notes
- Subcommand `--no-color` remains as an alias for `--color never`
- `--color auto` honors the `NO_COLOR` environment variable
- Clause breaks only apply to sentences that exceed `max_width`, and only at whitespace after the punctuation; tokens such as `1,000`, `10:30`, URLs, and `--flags` never split

- - -

## v0.8.1 - 2026-07-20
#### Bug Fixes
- (**pre-commit**) ship a real `id: snapper` hook (`snapper --in-place`) with extension-based `files` matching; drop leftover SemBr `id`/`entry` and invalid identify type `org`
- (**lockfile**) CI asserts `Cargo.toml` package version equals the `snapper-fmt` entry in `Cargo.lock` and that `cargo metadata --locked` succeeds (catches the 0.7.9 tag skew)
#### Features
- (**cli**) install `snapper-fmt` as a second binary name (same program as `snapper`) so hosts that already own openSUSE's `snapper` can call the formatter without PATH surgery
#### Documentation
- Installation: name collision with openSUSE snapper; prefer `snapper-fmt` on those systems
- Pre-commit section: which extensions the hook matches and that Rust is required to build it

- - -

## v0.8.0 - 2026-07-20
#### Features
- (**pandoc**) optional `--use-pandoc` path: parse with pandoc (CLI JSON or in-process FFI), classify regions from AST node kinds, reflow only prose (`Para`/`Plain`); structure (headers, code fences, tables, lists, math islands) preserved
- (**pandoc**) `--pandoc-backend auto|ffi|cli` (default auto: prefer warm FFI when available, else CLI); explicit `ffi` fails closed if the library is missing
- (**pandoc**) content-addressed AST cache (`SNAPPER_PANDOC_CACHE` / `SNAPPER_PANDOC_CACHE_DIR`)
- (**pandoc-colink**) opt-in Cargo feature: absorb `libsnapper_pandoc.a` from `native/snapper-pandoc/build-static.sh` (`ghc -staticlib`) into one `snapper` binary — no `SNAPPER_PANDOC_LIB` discovery, no `libHS*` RUNPATH graph; **not** enabled by default (default/cargo-dist remain GHC-free)
#### Bug Fixes
- (**org** / **latex**) keep full headline and sectioning lines as Structure (do not reflow multi-sentence titles; no orphan title lines without stars / mid-brace splits)
- (**markdown**) keep setext headings (title + `=`/`-` underline) as Structure so multi-sentence titles do not reflow and underlines are not glued onto title text
- (**ci**) exclude `pandoc-colink` from WASM feature matrix (`--all-features` panic without static archive); colink still fail-closed in main CI
#### Documentation
- Native flib README: static archive vs shared dlopen; howto orgmode pandoc backend; UPX is optional packer only (not used in release)
- Formats reference: Org headlines, LaTeX sectioning, Markdown ATX and setext headings are full Structure lines
#### Tests / CI
- AST fixture tests (JSON on disk) run without pandoc binary; live CLI/FFI parity tests skip cleanly when neither is installed; CI asserts colink without `.a` fails with archive message; default `cargo test` stays green without GHC
- Unit and format_text coverage for multi-sentence Org headlines, LaTeX `\section{...}` titles, and Markdown setext
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
