local MODREV, SPECREV = "scm", "-1"
rockspec_format = "3.0"
package = "snapper.nvim"
version = MODREV .. SPECREV

description = {
  summary = "Neovim plugin for snapper semantic line break formatter",
  detailed = [[
    Provides LSP integration, format-on-save, range formatting,
    and conform.nvim support for the snapper formatter.
  ]],
  labels = { "neovim", "formatter", "lsp" },
  homepage = "https://github.com/TurtleTech-ehf/snapper",
  license = "MIT",
}

dependencies = {
  "lua >= 5.1",
}

source = {
  url = "git://github.com/TurtleTech-ehf/snapper",
  dir = "snapper",
}

build = {
  type = "builtin",
  copy_directories = {
    "doc",
    "ftplugin",
    "plugin",
  },
}
