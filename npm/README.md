# Snapper MCP wrapper source

This directory contains wrapper source for Snapper's MCP server.
It is not published on npm and is not an installation channel.

## Supported installation

```bash
cargo install snapper-fmt --features mcp
snapper mcp
```

The MCP feature is optional and is not present in published release binaries.

## MCP client configuration

Add to your MCP configuration:

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
- **check_formatting** -- `would_reformat` (same as CLI `--check`) plus fused/wrap/long line diagnostics
- **split_sentences** -- Split text into individual sentences

## Links

- [Documentation](https://snapper.turtletech.us/docs/)
- [GitHub](https://github.com/TurtleTech-ehf/snapper)
