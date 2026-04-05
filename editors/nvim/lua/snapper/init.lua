---@class SnapperConfig
---@field cmd string Path to snapper binary
---@field autostart boolean Auto-start LSP
---@field format_on_save boolean Enable format on save
---@field format_on_save_opts table Options for format on save
---@field filetypes string[] Supported filetypes
---@field keymaps table|nil Keymap configuration
---@field conform_integration boolean Auto-register with conform.nvim

local M = {}

local config_module = require('snapper.config')

M.config = {}

function M.setup(opts)
  M.config = config_module.merge(opts)

  if vim.fn.executable(M.config.cmd) == 0 then
    vim.notify("snapper binary not found: " .. M.config.cmd, vim.log.levels.WARN)
    return
  end

  if M.config.autostart then
    require("snapper.lsp").setup(M.config)
  end

  if M.config.conform_integration then
    local ok, _ = pcall(require, "conform")
    if ok then
      require("snapper.conform").setup(M.config)
    end
  end

  M.create_commands()
end

function M.create_commands()
  vim.api.nvim_create_user_command("SnapperFormat", function()
    vim.lsp.buf.format({ name = "snapper" })
  end, { desc = "Format with snapper" })

  vim.api.nvim_create_user_command("SnapperFormatRange", function()
    vim.lsp.buf.format({ name = "snapper" })
  end, { desc = "Format range with snapper", range = true })

  vim.api.nvim_create_user_command("SnapperCheck", function()
    local filename = vim.api.nvim_buf_get_name(0)
    if filename == "" then
      vim.notify("[snapper] Buffer has no file name", vim.log.levels.WARN)
      return
    end
    vim.cmd("silent update")
    local output = vim.fn.system({ M.config.cmd, "--check", filename })
    if vim.v.shell_error == 0 then
      vim.notify("[snapper] File is already formatted", vim.log.levels.INFO)
    else
      vim.notify("[snapper] File needs formatting:\n" .. output, vim.log.levels.WARN)
    end
  end, { desc = "Check if formatting needed" })

  vim.api.nvim_create_user_command("SnapperRestart", function()
    require("snapper.lsp").restart()
  end, { desc = "Restart snapper LSP" })

  vim.api.nvim_create_user_command("SnapperInfo", function()
    M.show_info()
  end, { desc = "Show snapper info" })
end

function M.show_info()
  local info = {
    "Snapper Info",
    "============",
    "Binary: " .. M.config.cmd,
    "LSP: " .. (require("snapper.lsp").is_running() and "running" or "stopped"),
    "Filetypes: " .. table.concat(M.config.filetypes, ", "),
  }
  vim.api.nvim_echo({ { table.concat(info, "\n"), "Normal" } }, true, {})
end

function M.formatexpr()
  local params = vim.lsp.util.make_format_params({})
  local results = vim.lsp.buf_request_sync(0, "textDocument/formatting", params, 1000)
  if results then
    for _, resp in pairs(results) do
      if resp.result then
        vim.lsp.util.apply_text_edits(resp.result, 0, "utf-16")
        return 0
      end
    end
  end
  return 1
end

return M
