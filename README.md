![img](branding/logo/snapper_logo.png)


# Table of Contents

-   [About](#org68a0492)
    -   [Why?](#orga69ed32)
    -   [Design](#orgd39b73d)
-   [Installation](#org2c8d4c7)
-   [Usage](#org00caea3)
    -   [Supported formats](#org869780b)
    -   [Pre-commit hook](#org96fa9ff)
    -   [Emacs (Apheleia)](#org3687ef0)
    -   [Git smudge/clean filter](#orgfdf477d)
    -   [Vale integration](#orgbdb5ef3)
    -   [Project config](#org455ce7d)
-   [Documentation](#orgb1446c5)
-   [Development](#org2c5a35c)
    -   [Key dependencies](#org33f3310)
    -   [Conventions](#org9afe9ed)
-   [License](#org0aa36d7)


<a id="org68a0492"></a>

# About

A fast, format-aware semantic line break formatter. Reformats prose so each
sentence occupies its own line, producing minimal and meaningful git diffs when
collaborating on documents.


<a id="orga69ed32"></a>

## Why?

When multiple authors collaborate on a paper using Git, traditional line
wrapping at a fixed column width causes problems. A single word change can
trigger a diff that spans an entire paragraph. By breaking at sentence
boundaries instead, each edit affects only the sentence that changed.

This convention, often called "semantic linefeeds," enjoys longstanding support
from technical writers. Existing tools fall short: latexindent.pl only
handles LaTeX, SemBr requires Python and neural networks, and most lack
multi-format awareness. `snapper` solves this as a standalone Rust binary with no
runtime dependencies, handling Org-mode, LaTeX, Markdown, and plaintext.


<a id="orgd39b73d"></a>

## Design

`snapper` runs a three-stage pipeline:

-   **Parse:** Classify input into prose regions and structure regions
-   **Split:** Detect sentence boundaries in prose regions
-   **Emit:** Output each sentence on its own line

Structure regions (code blocks, math environments, tables, front matter,
drawers, comments) pass through unchanged. Sentence detection relies on Unicode
UAX #29 segmentation with abbreviation-aware post-processing that avoids false
breaks at titles (Dr., Prof.), references (Fig., Eq.), and Latin terms (e.g.,
i.e., et al.).


<a id="org2c8d4c7"></a>

# Installation

From source:

    cargo build --release
    # Binary at ./target/release/snapper

Or install directly:

    cargo install --path .

With Nix:

    nix build github:HaoZeke/snapper


<a id="org00caea3"></a>

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


<a id="org869780b"></a>

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


<a id="org96fa9ff"></a>

## Pre-commit hook

    - repo: https://github.com/HaoZeke/snapper
      rev: v0.1.0
      hooks:
        - id: snapper


<a id="org3687ef0"></a>

## Emacs (Apheleia)

    (with-eval-after-load 'apheleia
      (push '(snapper . ("snapper" "--format" "org")) apheleia-formatters)
      (push '(org-mode . snapper) apheleia-mode-alist))


<a id="orgfdf477d"></a>

## Git smudge/clean filter

Auto-format on commit, transparent to collaborators:

    git config filter.snapper.clean "snapper --format org"
    git config filter.snapper.smudge cat

Then add to `.gitattributes`:

    *.org filter=snapper


<a id="orgbdb5ef3"></a>

## Vale integration

`snapper` ships a vale style package for editor hints. Add to your `.vale.ini`:

    StylesPath = /path/to/snapper/vale
    [*.org]
    BasedOnStyles = snapper

For precise CI checks, use `snapper --check` directly.


<a id="org455ce7d"></a>

## Project config

Drop a `.snapperrc.toml` in your project root:

    extra_abbreviations = ["GROMACS", "LAMMPS", "DFT"]
    ignore = ["*.bib", "*.cls"]
    format = "org"
    max_width = 0

`snapper` walks up from the current directory to find it.


<a id="orgb1446c5"></a>

# Documentation

Build the docs site with:

    pixi run docbld


<a id="org2c5a35c"></a>

# Development


<a id="org33f3310"></a>

## Key dependencies

-   **Clap 4 (derive):** CLI argument parsing
-   **unicode-segmentation:** UAX #29 sentence boundaries
-   **regex:** Abbreviation and format pattern matching
-   **textwrap:** Optional line width limiting
-   **thiserror:** Typed error handling


<a id="org9afe9ed"></a>

## Conventions

We use `cocogitto` via `cog` to handle commit conventions.


### Readme

Construct the `readme` via:

    ./scripts/org_to_md.sh readme_src.org README.md


<a id="org0aa36d7"></a>

# License

MIT.

