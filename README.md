![img](branding/logo/snapper_logo.png)


# Table of Contents

-   [About](#about)
    -   [Why?](#why)
    -   [Design](#design)
-   [Installation](#installation)
-   [Usage](#usage)
    -   [MCP server (AI assistants)](#mcp)
    -   [Supported formats](#supported-formats)
    -   [Pre-commit hook](#pre-commit-hook)
    -   [Emacs (Apheleia)](#emacs)
    -   [VS Code](#vscode)
    -   [Neovim](#neovim)
    -   [Vim](#vim)
    -   [Obsidian](#obsidian)
    -   [Microsoft Word](#word)
    -   [Git smudge/clean filter](#git-filter)
    -   [Vale integration](#vale)
    -   [Project config](#project-config)
-   [Documentation](#documentation)
-   [Development](#development)
    -   [Key dependencies](#key-dependencies)
    -   [Conventions](#conventions)
-   [License](#license)


<a id="about"></a>

# About

A fast, format-aware semantic line break formatter.
Reformats prose so each sentence occupies its own line, producing minimal and meaningful git diffs when collaborating on documents.


<a id="why"></a>

## Why?

When multiple authors collaborate on a paper using Git, traditional line wrapping at a fixed column width causes problems.
A single word change can trigger a diff that spans an entire paragraph.
By breaking at sentence boundaries instead, each edit affects only the sentence that changed.

This convention, often called [semantic line breaks](https://sembr.org/), enjoys longstanding support from technical writers.
`snapper` is a deterministic formatter (UAX #29 plus abbreviation tables and optional nnsplit).
It is not [admk/sembr](https://github.com/admk/sembr) (learned clause breaks) and not [sembr/skills](https://github.com/sembr/skills) (agent rewrite instructions).
`latexindent.pl` covers LaTeX only; snapper is a standalone Rust binary for Org-mode, LaTeX, Markdown, RST, and plaintext.


<a id="design"></a>

## Design

`snapper` runs a three-stage pipeline:

-   **Parse:** Classify input into prose regions and structure regions
-   **Split:** Detect sentence boundaries in prose regions
-   **Emit:** Output each sentence on its own line

Math environments, tables, front matter, drawers, and non-source comments pass through unchanged.
Fenced and delimited source blocks are `Region::Code`: fence/open/close lines stay structure, non-comment code stays verbatim, and comment lines reflow at sentence boundaries when the language has a `[code.<lang>]` entry in `.snapperrc.toml` (`snapper init` seeds common languages).
Pass `--format-code` to also pipe each block body through an optional per-language `formatter` argv (missing binary, non-zero exit, and timeouts degrade to the reflowed body).
Sentence detection relies on Unicode UAX #29 segmentation with abbreviation-aware post-processing that avoids false breaks at titles (Dr., Prof.), references (Fig., Eq.), and Latin terms (e.g., i.e., et al.).
Org emphasis (`*bold*`, `/italic/`, `_underline_`, `+strike+`) is kept atomic so splits cannot open a pseudo-headline mid-span.
Markdown `*em*` / `**strong**` (CommonMark flanking) and GFM `~~strike~~` are kept atomic the same way.


<a id="installation"></a>

# Installation

Pre-built binary (fastest):

    cargo binstall snapper-fmt

Shell one-liner (Linux/macOS):

    curl -LsSf https://github.com/TurtleTech-ehf/snapper/releases/latest/download/snapper-fmt-installer.sh | sh

Homebrew:

    brew install TurtleTech-ehf/tap/snapper-fmt

pip:

    pip install snapper-fmt

conda-forge:

    conda install -c conda-forge snapper-fmt

Compile from source:

    cargo install snapper-fmt

Nix:

    nix build github:TurtleTech-ehf/snapper

The crate is `snapper-fmt` on all registries.
Each install ships two CLI names for the same program: `snapper` and `snapper-fmt`.

**Name collision:** [openSUSE snapper](https://github.com/openSUSE/snapper) is a different project (Btrfs/LVM snapshots) that also installs a `snapper` binary.
On systems where that tool already owns `/usr/bin/snapper`, call this formatter as `snapper-fmt`, or put the TurtleTech install path ahead of the system path.


<a id="usage"></a>

# Usage

Format a file (output to stdout):

    snapper paper.org

Format in place:

    snapper --in-place paper.org

Pipe through stdin (for editor integration):

    cat draft.org | snapper --format org

Check formatting without modifying (for CI):

    snapper --check paper.org paper.tex notes.md

Limit line width (wrap long sentences at word boundaries):

    snapper --max-width 80 paper.org

Break after independent-clause punctuation (comma, semicolon, colon, em dash).
With the default unlimited width this inserts a newline after every such mark that already has whitespace after it:

    snapper --clause-breaks paper.org

With `--max-width` set, overflowing sentences prefer those marks.
A one-clause sentence stays one line.

Preview changes as a unified diff before committing:

    snapper --diff paper.org

Compare two versions at the sentence level (whitespace reflow produces zero diff):

    snapper sdiff paper_v1.org paper_v2.org

Watch files and auto-reformat on save:

    snapper watch '*.org' 'sections/*.tex'

Initialize a project (generates config, pre-commit, gitattributes):

    snapper init


<a id="mcp"></a>

## MCP server (AI assistants)

Published `snapper` / `snapper-fmt` binaries include the MCP server (`mcp` is a default Cargo feature).
Start the stdio server:

    snapper mcp

Agents should call snapper MCP (`format_text`) or the `snapper` CLI instead of applying [sembr.org](https://sembr.org/) / [sembr/skills](https://github.com/sembr/skills) wrapping by hand.
`format_text` accepts `clause_breaks`, `range` (`start` / `end`, 1-indexed inclusive), and `max_width` (default 0).
From source with `--no-default-features`, rebuild with MCP:

    cargo install snapper-fmt --features mcp

Tools: `format_text`, `detect_format`, `check_formatting`, `split_sentences`.
Configuration guide (org source in-tree): `docs/orgmode/howto/mcp-integration.org`; HTML docs: <https://snapper.turtletech.us/docs/howto/mcp-integration/> .


<a id="supported-formats"></a>

## Supported formats

<table border="2" cellspacing="0" cellpadding="6" rules="groups" frame="hsides">


<colgroup>
<col  class="org-left" />

<col  class="org-left" />

<col  class="org-left" />
</colgroup>
<thead>
<tr>
<th scope="col" class="org-left">Format</th>
<th scope="col" class="org-left">Extensions</th>
<th scope="col" class="org-left">Structure / code handling</th>
</tr>
</thead>
<tbody>
<tr>
<td class="org-left">Org-mode</td>
<td class="org-left"><code>.org</code></td>
<td class="org-left">Drawers, tables, keywords; <code>#+BEGIN_SRC</code> comment reflow</td>
</tr>

<tr>
<td class="org-left">LaTeX</td>
<td class="org-left"><code>.tex</code>, <code>.latex</code></td>
<td class="org-left">Preamble, math; minted and lstlisting comment reflow</td>
</tr>

<tr>
<td class="org-left">Markdown</td>
<td class="org-left"><code>.md</code>, <code>.markdown</code></td>
<td class="org-left">Front matter, HTML; fenced blocks comment reflow when configured</td>
</tr>

<tr>
<td class="org-left">RST</td>
<td class="org-left"><code>.rst</code></td>
<td class="org-left">Directives, literals; <code>.. code-block::</code> comment reflow</td>
</tr>

<tr>
<td class="org-left">Plaintext</td>
<td class="org-left">everything else</td>
<td class="org-left">(none; all text treated as prose)</td>
</tr>
</tbody>
</table>


<a id="pre-commit-hook"></a>

## Pre-commit hook

    - repo: https://github.com/TurtleTech-ehf/snapper
      rev: v0.10.0
      hooks:
        - id: snapper


<a id="emacs"></a>

## Emacs (Apheleia)

    (with-eval-after-load 'apheleia
      (push '(snapper . ("snapper" "--format" "org")) apheleia-formatters)
      (push '(org-mode . snapper) apheleia-mode-alist))


<a id="vscode"></a>

## VS Code

Install [TurtleTech.snapper](https://marketplace.visualstudio.com/items?itemName=TurtleTech.snapper) from the VS Code Marketplace.
The extension uses the built-in LSP server for format-on-save, range formatting, diagnostics, and code actions.


<a id="neovim"></a>

## Neovim

With `lazy.nvim` (rocks support):

    {
      "TurtleTech-ehf/snapper",
      ft = { "org", "tex", "markdown", "rst" },
      config = function()
        vim.opt.runtimepath:append(
          vim.fn.stdpath("data") .. "/lazy/snapper/editors/nvim"
        )
        require("snapper").setup()
      end,
    }

Or with `rocks.nvim`:

    :Rocks install snapper.nvim


<a id="vim"></a>

## Vim

    Plug 'TurtleTech-ehf/snapper', { 'rtp': 'editors/vim' }

This provides `formatprg` support for automatic formatting with the `gq` operator.


<a id="obsidian"></a>

## Obsidian

Development preview; not listed in Community Plugins.
The WebAssembly plugin source lives in `editors/obsidian` and is available for development builds only.


<a id="word"></a>

## Microsoft Word

Development preview; not published in AppSource.
The WebAssembly add-in source and sideloading instructions live in `editors/word`.


<a id="git-filter"></a>

## Git smudge/clean filter

Auto-format on commit, transparent to collaborators:

    git config filter.snapper.clean "snapper --format org"
    git config filter.snapper.smudge cat

Then add to `.gitattributes`:

    *.org filter=snapper


<a id="vale"></a>

## Vale integration

`snapper` ships a vale style package for editor hints.
Add to your `.vale.ini`:

    StylesPath = /path/to/snapper/vale
    [*.org]
    BasedOnStyles = snapper

For precise CI checks, use `snapper --check` directly.


<a id="project-config"></a>

## Project config

Drop a `.snapperrc.toml` in your project root:

    extra_abbreviations = ["GROMACS", "LAMMPS", "DFT"]
    ignore = ["*.bib", "*.cls"]
    format = "org"
    max_width = 0
    clause_breaks = false

    [latex]
    verbatim_envs = ["Verbatim"]
    structure_envs = ["algorithm", "comment"]
    verbatim_commands = ["Verb"]

`snapper` walks up from the current directory to find it.
Missing `[latex]` keys keep the built-in minted/lstlisting/verbatim, equation/figure, and verb/lstinline lists.


<a id="documentation"></a>

# Documentation

Build the docs site with:

    pixi run docbld


<a id="development"></a>

# Development


<a id="key-dependencies"></a>

## Key dependencies

-   **Clap 4 (derive):** CLI argument parsing
-   **unicode-segmentation:** UAX #29 sentence boundaries
-   **regex:** Abbreviation and format pattern matching
-   **textwrap:** Optional line width limiting
-   **thiserror:** Typed error handling


<a id="conventions"></a>

## Conventions

We use `cocogitto` via `cog` to handle commit conventions.


### Readme

Construct the `readme` via:

    ./scripts/org_to_md.sh readme_src.org README.md


<a id="license"></a>

# License

MIT.

