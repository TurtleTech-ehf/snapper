# Snapper for Obsidian

Development preview; not listed in Community Plugins.

The plugin formats the active Markdown note or selection with the Snapper
WebAssembly build. It is source-only: GitHub releases do not contain an
installable Obsidian plugin artifact.

## Development

The reproducible build sequence lives in `.github/workflows/wasm.yml`. It builds
the root WebAssembly artifact, the wrapper under `packages/snapper-wasm`, and the
plugin in this directory.

After a development build, copy `main.js` and `manifest.json` into a local
`.obsidian/plugins/snapper/` directory and enable the plugin for that vault.

## License

MIT - TurtleTech ehf.
