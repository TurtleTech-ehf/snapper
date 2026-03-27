![img](branding/logo/snapper_logo.png)


# Table of Contents

-   [About](#org4308355)
    -   [Why?](#org9eba865)
    -   [Design](#orgd092db4)
-   [Installation](#org54aa1ec)
-   [Usage](#org2086455)
    -   [Supported formats](#org0f6ab09)
    -   [Pre-commit hook](#org58ca712)
    -   [Emacs (Apheleia)](#org6a8f366)
    -   [Git smudge/clean filter](#orgbb92d91)
    -   [Vale integration](#org4237538)
    -   [Project config](#org26a5820)
-   [Documentation](#orgd4b0eba)
-   [Development](#org564688e)
    -   [Key dependencies](#orgbd35161)
    -   [Conventions](#org56d2cfc)
-   [License](#org769c847)


<a id="org4308355"></a>

# About

A fast, format-aware semantic line break formatter.
Reformats prose so each sentence occupies its own line, producing minimal and meaningful git diffs when collaborating on documents.


<a id="org9eba865"></a>

## Why?

When multiple authors collaborate on a paper using Git, traditional line wrapping at a fixed column width causes problems.
A single word change can trigger a diff that spans an entire paragraph.
By breaking at sentence boundaries instead, each edit affects only the sentence that changed.

This convention, often called "semantic linefeeds," enjoys longstanding support from technical writers.
Existing tools fall short: latexindent.pl only handles LaTeX, SemBr requires Python and neural networks, and most lack multi-format awareness.
`snapper` solves this as a standalone Rust binary with no runtime dependencies, handling Org-mode, LaTeX, Markdown, and plaintext.


<a id="orgd092db4"></a>

## Design

`snapper` runs a three-stage pipeline:

-   **Parse:** Classify input into prose regions and structure regions
-   **Split:** Detect sentence boundaries in prose regions
-   **Emit:** Output each sentence on its own line

Structure regions (code blocks, math environments, tables, front matter, drawers, comments) pass through unchanged.
Sentence detection relies on Unicode UAX #29 segmentation with abbreviation-aware post-processing that avoids false breaks at titles (Dr., Prof.), references (Fig., Eq.), and Latin terms (e.g., i.e., et al.).


<a id="org54aa1ec"></a>

# Installation

From [crates.io](https://crates.io/crates/snapper-fmt) (the crate is `snapper-fmt`, the binary it installs is `snapper`):

    cargo install snapper-fmt

From source:

    cargo build --release
    # Binary at ./target/release/snapper

With Nix:

    nix build github:TurtleTech-ehf/snapper


<a id="org2086455"></a>

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

Preview changes as a unified diff before committing:

    snapper --diff paper.org


<a id="org0f6ab09"></a>

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
<th scope="col" class="org-left">Structure preserved</th>
</tr>
</thead>
<tbody>
<tr>
<td class="org-left">Org-mode</td>
<td class="org-left"><code>.org</code></td>
<td class="org-left">Blocks, drawers, tables, keywords</td>
</tr>

<tr>
<td class="org-left">LaTeX</td>
<td class="org-left"><code>.tex</code>, <code>.latex</code></td>
<td class="org-left">Preamble, math, environments, comments</td>
</tr>

<tr>
<td class="org-left">Markdown</td>
<td class="org-left"><code>.md</code>, <code>.markdown</code></td>
<td class="org-left">Code blocks, front matter, HTML</td>
</tr>

<tr>
<td class="org-left">Plaintext</td>
<td class="org-left">everything else</td>
<td class="org-left">(none; all text treated as prose)</td>
</tr>
</tbody>
</table>


<a id="org58ca712"></a>

## Pre-commit hook

    - repo: https://github.com/TurtleTech-ehf/snapper
      rev: v0.1.0
      hooks:
        - id: snapper


<a id="org6a8f366"></a>

## Emacs (Apheleia)

    (with-eval-after-load 'apheleia
      (push '(snapper . ("snapper" "--format" "org")) apheleia-formatters)
      (push '(org-mode . snapper) apheleia-mode-alist))


<a id="orgbb92d91"></a>

## Git smudge/clean filter

Auto-format on commit, transparent to collaborators:

    git config filter.snapper.clean "snapper --format org"
    git config filter.snapper.smudge cat

Then add to `.gitattributes`:

    *.org filter=snapper


<a id="org4237538"></a>

## Vale integration

`snapper` ships a vale style package for editor hints.
Add to your `.vale.ini`:

    StylesPath = /path/to/snapper/vale
    [*.org]
    BasedOnStyles = snapper

For precise CI checks, use `snapper --check` directly.


<a id="org26a5820"></a>

## Project config

Drop a `.snapperrc.toml` in your project root:

    extra_abbreviations = ["GROMACS", "LAMMPS", "DFT"]
    ignore = ["*.bib", "*.cls"]
    format = "org"
    max_width = 0

`snapper` walks up from the current directory to find it.


<a id="orgd4b0eba"></a>

# Documentation

Build the docs site with:

    pixi run docbld


<a id="org564688e"></a>

# Development


<a id="orgbd35161"></a>

## Key dependencies

-   **Clap 4 (derive):** CLI argument parsing
-   **unicode-segmentation:** UAX #29 sentence boundaries
-   **regex:** Abbreviation and format pattern matching
-   **textwrap:** Optional line width limiting
-   **thiserror:** Typed error handling


<a id="org56d2cfc"></a>

## Conventions

We use `cocogitto` via `cog` to handle commit conventions.


### Readme

Construct the `readme` via:

    ./scripts/org_to_md.sh readme_src.org README.md


<a id="org769c847"></a>

# License

MIT.

