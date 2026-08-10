# Snapper MCP wrapper source

This directory contains wrapper source for Snapper's MCP server.
It is not published on npm and is not an installation channel.

## Supported installation

```bash
snapper mcp
```

Published `snapper` / `snapper-fmt` binaries include the MCP server (`mcp` is a default Cargo feature).
Agents should call this server or the `snapper` CLI instead of applying [sembr.org](https://sembr.org/) / [sembr/skills](https://github.com/sembr/skills) wrapping by hand.

From source with `--no-default-features`, rebuild with MCP:

```bash
cargo install snapper-fmt --features mcp
```

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

- **format_text** -- Format text with semantic line breaks (supports Org, LaTeX, Markdown, RST, plaintext; `clause_breaks`, `range`, `max_width`)
- **detect_format** -- Detect document format from content
- **check_formatting** -- `would_reformat` (same as CLI `--check`) plus fused/wrap/long line diagnostics
- **split_sentences** -- Split text into individual sentences

## Links

- [Documentation](https://snapper.turtletech.us/docs/)
- [GitHub](https://github.com/TurtleTech-ehf/snapper)
