# snapper - Semantic Line Breaks

Format prose so each sentence occupies its own line, producing clean git diffs for collaborative academic writing.

## Quick Start

1. **Install the `snapper` binary** (see [Requirements](#requirements) below)
2. **Install this extension** from the VS Code Marketplace
3. Open a Markdown, LaTeX, Org, RST, or plain text file
4. Run **snapper: Format Document** from the command palette (`Shift+Alt+F`)

To format on save, add to your `settings.json`:

```json
{
  "editor.formatOnSave": true,
  "[markdown]": { "editor.defaultFormatter": "TurtleTech.snapper" },
  "[latex]": { "editor.defaultFormatter": "TurtleTech.snapper" },
  "[org]": { "editor.defaultFormatter": "TurtleTech.snapper" }
}
```

## Features

- **Format on save** via the built-in LSP server
- **Range formatting** -- format just the selected text
- **Diagnostics** -- flags lines with multiple sentences as hints
- **Quick fixes** -- code actions to split multi-sentence lines
- **Project config** -- reads `.snapperrc.toml` for abbreviations, max width, per-format overrides
- **Format-aware** -- understands Org-mode, LaTeX, Markdown, RST, and plaintext
- **Abbreviation-aware** -- handles Dr., Fig., Eq., e.g., i.e., et al. and 80+ more
- **Status bar** -- shows whether the LSP server is running
- **Output channel** -- check Output > snapper for logs and diagnostics

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

# Nix
nix build github:TurtleTech-ehf/snapper

# Compile from source
cargo install snapper-fmt
```

The crate is `snapper-fmt`; the binary it installs is `snapper`.

If the binary is not on your PATH, the extension will show an error with a link to the install guide. You can also set a custom path via the `snapper.path` setting.

## Commands

| Command | Description |
|---------|-------------|
| `snapper: Format Document with snapper` | Format the active document |
| `snapper: Check Current File` | Check if the file needs formatting |
| `snapper: Preview Formatting Changes` | Show a diff of what would change |
| `snapper: Initialize Project` | Create `.snapperrc.toml` and `.gitattributes` |
| `snapper: Restart Language Server` | Restart the snapper LSP server |
| `snapper: Show Output` | Open the snapper output channel |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `snapper.path` | `snapper` | Path to the snapper binary |
| `snapper.trace.server` | `off` | Trace LSP communication (`off`, `messages`, `verbose`) |

## Project configuration

The LSP server reads `.snapperrc.toml` from your workspace root (or any parent directory).
Run **snapper: Initialize Project** to generate one.
Changes to `.snapperrc.toml` are picked up automatically -- no restart needed.

```toml
extra_abbreviations = ["GROMACS", "LAMMPS", "DFT"]
ignore = ["*.bib", "*.cls"]
format = "org"
max_width = 80

[latex]
extra_abbreviations = ["Thm", "Lem"]
max_width = 100
```

See the [configuration reference](https://snapper.turtletech.us/docs/reference/config.html) for all options.

## Supported languages

- Org-mode (`.org`)
- LaTeX (`.tex`)
- Markdown (`.md`)
- reStructuredText (`.rst`)
- Plaintext (`.txt`)

## VS Code OSS / VSCodium

This extension is published to the Microsoft Marketplace. If you use
VSCodium, code-oss (Arch Linux), or another OSS build that cannot access
the Marketplace:

1. Download the `.vsix` file from
   [GitHub Releases](https://github.com/TurtleTech-ehf/snapper/releases)
2. Install manually:
   ```bash
   codium --install-extension snapper-VERSION.vsix
   # or for Arch code-oss:
   code-oss --install-extension snapper-VERSION.vsix
   ```
3. You can also install from the Extensions view: click the `...` menu,
   select "Install from VSIX...", and pick the downloaded file.

## Troubleshooting

**"snapper binary not found"**

The extension needs the `snapper` CLI on your PATH.
Run `snapper --version` in a terminal to verify.
If installed to a non-standard location, set `snapper.path` in VS Code settings.

**Extension seems inactive / no formatting**

1. Check the status bar (bottom-right) for the snapper indicator
2. Open the Output panel (`View > Output`) and select "snapper" from the dropdown
3. Run "snapper: Restart Language Server" from the command palette

**Formatting does not run on save**

VS Code requires both `editor.formatOnSave: true` and the default formatter
set to `TurtleTech.snapper` for the relevant language.
See the [Quick Start](#quick-start) section above.

## Links

- [Documentation](https://snapper.turtletech.us/docs/)
- [GitHub](https://github.com/TurtleTech-ehf/snapper)
- [crates.io](https://crates.io/crates/snapper-fmt)
- [PyPI](https://pypi.org/project/snapper-fmt/)
