# Snapper for Microsoft Word

Office add-in that formats document prose with **semantic line breaks** (one sentence per line) using the **snapper WASM** build (`@snapper/wasm`). Useful when drafting in Word and exporting to Org, Markdown, or LaTeX for git-friendly diffs.

**Version:** 0.10.0 (tracks snapper-fmt)

Development preview; not published in AppSource.

## Features

- **Format Document** — rewrites each non-empty paragraph through snapper (plaintext mode).
- **Format Selection** — same for the current selection.
- **Max width** — optional soft wrap (0 = unlimited).
- **Language** — abbreviation sets (`en`, `de`, `fr`, `is`, `pl`) plus optional extra abbreviations.
- Runs in the add-in via WASM (no local `snapper` binary required).

## Development (sideload)

Prerequisites: Node 20+, Word desktop or Word on the web with sideloading enabled.

```bash
# From repo root — build WASM package (wasm-pack or CI artifact in packages/snapper-wasm/pkg)
cd packages/snapper-wasm && npm install && npm run build && cd -

cd editors/word
npm install
npm run dev
# Serves taskpane on https://localhost:3000 (see webpack.config.js)
```

Sideload `manifest.xml` in Word (Insert → Add-ins → My Add-ins → Upload My Add-in). For production, point manifest URLs at an HTTPS host serving webpack `dist/`.

CI builds the Word add-in in `.github/workflows/wasm.yml` (`build-word` job).

## Relationship to the CLI

Word uses the **plaintext** format path in WASM. For Org/LaTeX fidelity, prefer the CLI or VS Code extension (LSP). Delimiter-span and abbreviation behavior matches snapper **0.8.1** as exposed by the WASM API.

## License

MIT — TurtleTech ehf.
