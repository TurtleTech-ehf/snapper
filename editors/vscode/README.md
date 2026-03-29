# snapper - Semantic Line Breaks

Format prose so each sentence occupies its own line, producing clean git diffs for collaborative academic writing.

## Features

- **Format on save** via the built-in LSP server
- **Range formatting** -- format just the selected text
- **Diagnostics** -- flags lines with multiple sentences as hints
- **Format-aware** -- understands Org-mode, LaTeX, Markdown, RST, and plaintext
- **Abbreviation-aware** -- handles Dr., Fig., Eq., e.g., i.e., et al. and 80+ more

## How it works

Given a paragraph like this:

```
This is the first sentence. It continues with more details about the topic. See Fig. 3 for the results.
```

snapper produces:

```
This is the first sentence.
It continues with more details about the topic.
See Fig. 3 for the results.
```

Each sentence on its own line. A one-word edit produces a one-line diff instead of reflowing the entire paragraph.

## Requirements

The `snapper` binary must be installed and on your PATH.

```bash
# Pre-built binary (fastest)
cargo binstall snapper-fmt

# Shell installer (Linux/macOS)
curl -LsSf https://github.com/TurtleTech-ehf/snapper/releases/latest/download/snapper-fmt-installer.sh | sh

# pip
pip install snapper-fmt

# Homebrew
brew install TurtleTech-ehf/tap/snapper-fmt

# Compile from source
cargo install snapper-fmt
```

The crate is `snapper-fmt`; the binary it installs is `snapper`.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `snapper.path` | `snapper` | Path to the snapper binary |
| `snapper.formatOnSave` | `false` | Format files with snapper on save |

## Supported languages

- Org-mode (`.org`)
- LaTeX (`.tex`)
- Markdown (`.md`)
- reStructuredText (`.rst`)
- Plaintext (`.txt`)

## Links

- [Documentation](https://snapper.turtletech.us/docs/)
- [GitHub](https://github.com/TurtleTech-ehf/snapper)
- [crates.io](https://crates.io/crates/snapper-fmt)
- [PyPI](https://pypi.org/project/snapper-fmt/)
