# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## v0.7.5 - 2026-04-09
#### Bug Fixes
- (**cog**) bump all package versions in pre_bump_hooks, drop stale pyproject sed - (7edca39) - *HaoZeke*
- (**markdown**) preserve pipe-delimited tables as structure regions - (e745591) - *HaoZeke*
- (**version**) sync all package versions to 0.7.4 - (839ce57) - *HaoZeke*

- - -

## v0.6.0 - 2026-04-04
#### Features
- (**lsp**) add features now that it works - (15a285f) - *HaoZeke*
- (**vsx**) fix lsp extension - (f7f222d) - *HaoZeke*
#### Miscellaneous Chores
- (**ci**) remove gratuitous pointless checks - (3c30012) - *HaoZeke*
- (**clippy**) apply - (32c0f9f) - *HaoZeke*
- (**cog**) setup for releasing vsx - (317023c) - *HaoZeke*
- (**fmt**) set edition for aphelia - (3507bad) - *HaoZeke*
- (**lint**) apply - (c0a84c1) - *HaoZeke*
- (**lsp**) reformat and be more targetted - (5d4d01f) - *HaoZeke*
- (**vscodeignore**) initialize - (72545ce) - *HaoZeke*

- - -

## v0.5.0 - 2026-03-30
#### Maintenance
- code quality improvements across the codebase - (5df1ac6) - *HaoZeke*
#### Features
- (**vscode**) overhaul extension UX and fix LSP config loading - (70a9e1e) - *HaoZeke*
- add Antics analytics tracker - (5aad5e1) - *HaoZeke*
- publish VS Code extension to marketplace (TurtleTech.snapper v0.4.0) - (40618d1) - *HaoZeke*
#### Bug Fixes
- add security headers for CF Pages - (83161fe) - *HaoZeke*
- add README to VS Code extension for marketplace listing - (fa333c1) - *HaoZeke*
- remove stale wdiff/word-level references from docs and CLI reference, update editor integration feature card for VS Code marketplace - (b5d5296) - *HaoZeke*

- - -

## v0.4.0 - 2026-03-29
#### Documentation
- add pandoc backend and word-diff howto guides, update CLI reference, fix landing page feature descriptions, remove SvelteKit build artifacts - (f25febb) - *HaoZeke*
#### Features
- pandoc AST backend + word-level diff (latexdiff-style) - (f377e2b) - *HaoZeke*
#### Bug Fixes
- bare file:/http: links treated as structure in Org parser, URLs/file paths protected in inline token regex - (d1b492b) - *HaoZeke*
- remove 30MB of SvelteKit build artifacts from repo, gitignore node_modules/.svelte-kit/build - (b32ce42) - *HaoZeke*
#### Refactoring
- remove wdiff/markup/compile (belongs in separate snapdiff tool), keep pandoc parser and clap-markdown CLI docs - (984ffdc) - *HaoZeke*

- - -

## v0.3.2 - 2026-03-28
#### Documentation
- comprehensive update -- all install methods, VS Code extension, GitHub Action, SARIF, git-diff/sdiff/watch/lsp subcommands, RST, multi-lang abbreviations, correct project structure - (93545dc) - *HaoZeke*
- fix stale docs (FAQ, abbreviations, config, CLI) -- add multi-lang support, RST, all subcommands, correct FormatConfig fields - (b9f0686) - *HaoZeke*
- add RST to formats reference, update landing page install section (binstall/brew/pip/shell), mention RST in feature cards - (1696339) - *HaoZeke*
#### Features
- VS Code extension scaffold, fix PyPI aarch64 manylinux_2_28 for ring crate - (7d7af5e) - *HaoZeke*
- add RST format support (line-based parser for directives, literal blocks, sections, field lists, tables) - (058abe7) - *HaoZeke*
#### Bug Fixes
- move pre_bump_hooks to top-level in cog.toml (was under [changelog] table) - (d3f6048) - *HaoZeke*
- remove unnecessary pragma workaround from FAQ Dr. Smith headline - (87d85b0) - *HaoZeke*
#### Miscellaneous Chores
- stage updated .po files from doc changes - (a47f609) - *HaoZeke*
- cog pre_bump_hooks to sync pyproject.toml version - (cd054bb) - *HaoZeke*

- - -

## v0.3.1 - 2026-03-28
#### Bug Fixes
- proper maturin wheel CI matching readcon-core pattern (sdist + manylinux wheels + OIDC publish) - (10d8c63) - *HaoZeke*
#### Miscellaneous Chores
- bump to 0.3.1 for PyPI first publish - (08f5c2c) - *HaoZeke*

- - -

## v0.3.0 - 2026-03-28
#### Documentation
- LSP editor integration docs (Neovim, Emacs eglot, VS Code, Helix), translated quickstart updates - (ce60558) - *HaoZeke*
- update CLI reference for v0.2, add neural detection and pragmas howto guides - (48afc73) - *HaoZeke*
#### Features
- multi-language abbreviations (de/fr/is/pl), proper TOML config with serde, git-diff, GitHub Action, Homebrew tap, pip/maturin package - (af2c2cd) - *HaoZeke*
- git-diff, proper TOML config, GitHub Action, Homebrew tap, pip package - (b52bd83) - *HaoZeke*
- LSP server (snapper lsp) with formatting, range formatting, and diagnostics - (8c44eea) - *HaoZeke*
- sdiff demo tab on landing page, updated quickstart and README with sdiff/watch/init - (e59ecc3) - *HaoZeke*
- sentence-level diff (snapper sdiff) and watch mode (snapper watch) - (d241c1e) - *HaoZeke*
- add lychee link checker, typos spell check, and dogfood CI; close snapper-3sx - (f25a6d5) - *HaoZeke*
- SEO audit fixes (meta/OG/Twitter/JSON-LD/sitemap/robots), 3 new feature cards, compressed logo, v0.2 feature i18n - (7708fdf) - *HaoZeke*
- neural sentence detection via nnsplit (always-on, no feature flag) - (0322a17) - *HaoZeke*
#### Bug Fixes
- binstall metadata (correct archive names and format), add PyPI maturin wheel CI - (e8e46be) - *HaoZeke*
#### Miscellaneous Chores
- bump to 0.3.0 - (1611299) - *HaoZeke*
- gitignore overrides for lychee and typos configs - (fdaa53b) - *HaoZeke*

- - -

## v0.2.1 - 2026-03-27
#### Features
- add cargo-dist release CI for cross-platform binaries and binstall support - (dfffb56) - *HaoZeke*
#### Miscellaneous Chores
- bump to 0.2.1 for first binary release - (d9c591b) - *HaoZeke*

- - -

## v0.2.0 - 2026-03-27
#### Documentation
- full Sphinx docs structure matching bless pattern - (9960580) - *HaoZeke*
- document snapper-fmt crate name, add crate-level docs - (951da2f) - *HaoZeke*
#### Features
- add 7 new features for v0.2 - (40cef57) - *HaoZeke*
- translate doc headers and nav for Icelandic and Polish, gettext .po structure for incremental prose translation - (4df21a4) - *HaoZeke*
- multi-language docs (en/is/pl) via Sphinx gettext + Shibuya language selector - (9785ec6) - *HaoZeke*
- favicons for landing page and sphinx docs - (4c38882) - *HaoZeke*
- i18n (en/is/pl) with locale detection, fix docs logo link to landing page - (e9d4757) - *HaoZeke*
- switch to Cloudflare Pages, add pixi deploy task with docs build - (b1c6b12) - *HaoZeke*
- light/dark toggle with system preference, Umami analytics on landing page - (ccb7d2b) - *HaoZeke*
- bigger logo, docs link, Umami analytics, Shibuya color scheme - (393a2bc) - *HaoZeke*
- deploy landing page to snapper.rgoswami.me via GitHub Pages - (b346115) - *HaoZeke*
#### Bug Fixes
- repair broken .po header line endings from translate script - (fe3818c) - *HaoZeke*
- gitignore build artifacts, fix DNS to CF Pages instead of GitHub Pages - (fdcff70) - *HaoZeke*
- hermetic ox-rst install via git clone (no MELPA dependency) - (7d21601) - *HaoZeke*
#### Miscellaneous Chores
- bump version to 0.2.0 - (d71832c) - *HaoZeke*
- transfer to TurtleTech-ehf, switch site to snapper.turtletech.us - (e4f37b7) - *HaoZeke*
- rename crate to snapper-fmt for crates.io (binary stays snapper) - (0590fc0) - *HaoZeke*

- - -

## v0.1.0 - 2026-03-27
#### Features
- add landing page with interactive before/after diff demo - (e7d4dab) - *HaoZeke*
- fix trailing newlines, CRLF, list continuations, config abbreviations, inline placeholders - (dba1730) - *HaoZeke*
- initial release of snapper - (f5b2d8c) - *HaoZeke*
#### Miscellaneous Chores
- replace bloated Canva SVG with clean 4KB vector recreation - (7675a05) - *HaoZeke*

- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).