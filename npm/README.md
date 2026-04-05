# @turtletech/snapper-mcp

MCP (Model Context Protocol) server for [snapper](https://snapper.turtletech.us), the semantic line break formatter.

## Quick start

```bash
npm install -g @turtletech/snapper-mcp
```

This installs a wrapper that runs `snapper mcp` on stdin/stdout. The `snapper` binary must be available in your PATH.

On `npm install`, the package attempts to install snapper automatically via `cargo-binstall` or `cargo install`. If that fails, install manually:

```bash
cargo binstall snapper-fmt
# or from source with MCP support:
cargo install snapper-fmt --features mcp
# or via pip:
pip install snapper-fmt
```

## Claude Desktop / Claude Code

Add to your MCP configuration:

```json
{
  "mcpServers": {
    "snapper": {
      "command": "npx",
      "args": ["@turtletech/snapper-mcp"]
    }
  }
}
```

Or if snapper is already on your PATH:

```json
{
  "mcpServers": {
    "snapper": {
      "command": "snapper",
      "args": ["mcp"]
    }
  }
}
```

## Tools

- **format_text** -- Format text with semantic line breaks (supports Org, LaTeX, Markdown, RST, plaintext)
- **detect_format** -- Detect document format from content
- **check_formatting** -- Find lines with multiple sentences
- **split_sentences** -- Split text into individual sentences

## Links

- [Documentation](https://snapper.turtletech.us/docs/)
- [GitHub](https://github.com/TurtleTech-ehf/snapper)
