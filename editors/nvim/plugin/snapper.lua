-- Plugin entry point for non-lazy loading
if vim.fn.has("nvim-0.10") == 0 then
  vim.notify("snapper.nvim requires Neovim 0.10+", vim.log.levels.ERROR)
  return
end

-- Commands are created in setup(), but provide a minimal :Snapper command
-- that prompts to run setup()
vim.api.nvim_create_user_command("Snapper", function()
  vim.notify("Please run require('snapper').setup() in your config", vim.log.levels.INFO)
end, {
  desc = "Snapper (requires setup)",
  nargs = "?",
  complete = function()
    return { "format", "check", "restart", "info" }
  end,
})