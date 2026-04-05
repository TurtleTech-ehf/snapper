# snapper.nvim

Neovim plugin for [snapper](https://github.com/TurtleTech-ehf/snapper) - the semantic line break formatter.

## Features

- Full LSP integration (formatting, diagnostics, code actions)
- Format-on-save support
- Range formatting
- conform.nvim integration
- Binary auto-detection
- Vim compatibility via formatprg/formatexpr

## Requirements

- Neovim 0.10+
- snapper binary installed

## Installation

### lazy.nvim

```lua
{
  "TurtleTech-ehf/snapper",
  dependencies = {
    "stevearc/conform.nvim",  -- Optional: for conform integration
  },
  ft = { "org", "tex", "markdown", "rst", "plaintex" },
  config = function()
    vim.opt.runtimepath:append(
      vim.fn.stdpath("data") .. "/lazy/snapper/editors/nvim"
    )
    require("snapper").setup({
      -- Configuration options (see below)
    })
  end,
}
```

### rocks.nvim

```vim
:Rocks install snapper.nvim
```

### vim-plug

```vim
Plug 'TurtleTech-ehf/snapper', { 'rtp': 'editors/nvim' }
```

## Configuration

```lua
require("snapper").setup({
  cmd = "snapper",  -- Path to binary
  autostart = true,  -- Auto-start LSP on supported filetypes
  format_on_save = false,  -- Enable format on save
  format_on_save_opts = {
    timeout_ms = 1000,
    async = false,
  },
  filetypes = { "org", "tex", "markdown", "rst", "plaintext" },  -- Supported filetypes
  keymaps = {  -- Set to nil to disable keymaps
    format = "<leader>sf",     -- Format buffer
    format_range = "<leader>sF", -- Format selection
    check = "<leader>sc",      -- Check formatting
  },
  conform_integration = true,  -- Auto-register with conform.nvim
})
```

## Commands

- `:SnapperFormat` - Format current buffer
- `:SnapperFormatRange` - Format visual selection  
- `:SnapperCheck` - Check if formatting needed
- `:SnapperRestart` - Restart LSP server
- `:SnapperInfo` - Show plugin status

## Integration

### With conform.nvim

If you have `conform.nvim` installed and `conform_integration = true` in your config, snapper will be automatically registered. Add it to your conform config:

```lua
require("conform").setup({
  formatters_by_ft = {
    org = { "snapper" },
    tex = { "snapper" },
    markdown = { "snapper" },
    rst = { "snapper" },
  },
})
```

### With nvim-lspconfig

The snapper LSP server can also be configured manually with nvim-lspconfig:

```lua
require("lspconfig").snapper.setup({
  cmd = { "snapper", "lsp" },
  filetypes = { "org", "tex", "markdown", "rst" },
  root_dir = require("lspconfig").util.root_pattern(".snapperrc.toml", ".git"),
})
```

## Vim Compatibility

For Vim 8+ or older Neovim installations, the plugin also supports traditional Vim formatting via `formatprg` and `formatexpr`.

## License

MIT