# Command-Line Help for `snapper`

This document contains the help content for the `snapper` command-line program.

**Command Overview:**

* [`snapper`↴](#snapper)
* [`snapper init`↴](#snapper-init)
* [`snapper sdiff`↴](#snapper-sdiff)
* [`snapper git-diff`↴](#snapper-git-diff)
* [`snapper lsp`↴](#snapper-lsp)
* [`snapper mcp`↴](#snapper-mcp)
* [`snapper watch`↴](#snapper-watch)

## `snapper`

Semantic line break formatter

**Usage:** `snapper [OPTIONS] [FILES]... [COMMAND]`

###### **Subcommands:**

* `init` — Initialize snapper for a project (generate config, pre-commit, gitattributes)
* `sdiff` — Sentence-level diff between two files
* `git-diff` — Sentence-level diff against a git ref
* `lsp` — Start the LSP server (stdin/stdout)
* `mcp` — Start the MCP server (stdin/stdout)
* `watch` — Watch files and reformat on change

###### **Arguments:**

* `<FILES>` — Input files. Reads stdin if omitted

###### **Options:**

* `-f`, `--format <FORMAT>` — Input format (auto-detected from extension if omitted)

  Possible values: `org`, `latex`, `markdown`, `rst`, `plaintext`

* `--stdin-filepath <STDIN_FILEPATH>` — Assume this filename when reading stdin (for format auto-detection)
* `-o`, `--output <OUTPUT>` — Output file (stdout if omitted)
* `-i`, `--in-place` — Modify files in place
* `-w`, `--max-width <MAX_WIDTH>` — Maximum line width (0 = unlimited)

  Default value: `0`
* `--clause-breaks` — Prefer soft breaks after independent-clause punctuation (comma, semicolon, colon, em dash). With `--max-width 0` (the default), insert a newline after every such mark that is already followed by whitespace. With `--max-width` set, prefer those marks when wrapping an overflowing sentence

  Default value: `false`
* `--neural` — Use neural sentence detection (nnsplit LSTM model)
* `--lang <LANG>` — Language for neural sentence detection (default: en). Available: en, de, fr, no, sv, zh, tr, ru, uk
* `--model-path <MODEL_PATH>` — Path to custom ONNX model file for neural detection
* `--use-pandoc` — Use pandoc as parser backend (universal format support)
* `--pandoc-backend <BACKEND>` — Pandoc AST source when `--use-pandoc` is set: `auto` (prefer in-process FFI, else CLI), `ffi` (`libsnapper_pandoc`), or `cli` (`pandoc` subprocess). Default: `auto`. `ffi` fails explicitly if the library is missing

  Default value: `auto`
* `--check` — Exit with code 1 if any file would change
* `--strict-long` — Treat advisory `long` diagnostics as `--check` failures

  Default value: `false`
* `--diff` — Show a unified diff of what would change
* `--config <CONFIG>` — Path to config file (default: .snapperrc.toml in current or parent dirs)
* `--range <RANGE>` — Only format lines in this range (1-indexed, inclusive). Format: START:END
* `--output-format <OUTPUT_FORMAT>` — Output format for --check mode

  Default value: `text`

  Possible values: `text`, `json`, `sarif`

* `--format-code` — Pipe each code block's body through the per-language formatter configured under `[code.<lang>.formatter]` in `.snapperrc.toml`. The formatter runs after the in-block comment reflow. Missing binaries, non-zero exits, and timeouts surface as stderr diagnostics; snapper still exits 0

  Default value: `false`



## `snapper init`

Initialize snapper for a project (generate config, pre-commit, gitattributes)

**Usage:** `snapper init [OPTIONS]`

###### **Options:**

* `--dry-run` — Preview what would be generated without writing files



## `snapper sdiff`

Sentence-level diff between two files

**Usage:** `snapper sdiff [OPTIONS] <OLD> <NEW>`

###### **Arguments:**

* `<OLD>` — Original file
* `<NEW>` — Modified file

###### **Options:**

* `-f`, `--format <FORMAT>` — Input format (auto-detected from extension if omitted)

  Possible values: `org`, `latex`, `markdown`, `rst`, `plaintext`

* `--no-color` — Disable colored output



## `snapper git-diff`

Sentence-level diff against a git ref

**Usage:** `snapper git-diff [OPTIONS] [GIT_REF] [FILES]...`

###### **Arguments:**

* `<GIT_REF>` — Git ref to compare against (default: HEAD)

  Default value: `HEAD`
* `<FILES>` — Files to diff. If omitted, diffs all changed prose files

###### **Options:**

* `-f`, `--format <FORMAT>` — Input format (auto-detected from extension if omitted)

  Possible values: `org`, `latex`, `markdown`, `rst`, `plaintext`

* `--no-color` — Disable colored output



## `snapper lsp`

Start the LSP server (stdin/stdout)

**Usage:** `snapper lsp`



## `snapper mcp`

Start the MCP server (stdin/stdout)

**Usage:** `snapper mcp`



## `snapper watch`

Watch files and reformat on change

**Usage:** `snapper watch [OPTIONS] <PATTERNS>...`

###### **Arguments:**

* `<PATTERNS>` — Files or glob patterns to watch

###### **Options:**

* `-f`, `--format <FORMAT>` — Input format (auto-detected from extension if omitted)

  Possible values: `org`, `latex`, `markdown`, `rst`, `plaintext`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
