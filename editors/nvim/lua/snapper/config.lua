-- Configuration module for snapper.nvim
local M = {}

-- Default configuration
M.defaults = {
  cmd = "snapper",
  autostart = true,
  format_on_save = false,
  format_on_save_opts = {
    timeout_ms = 1000,
    async = false,
  },
  filetypes = { "org", "tex", "markdown", "rst", "plaintext" },
  keymaps = {
    format = "<leader>sf",     -- Format buffer
    format_range = "<leader>sF", -- Format selection
    check = "<leader>sc",      -- Check formatting
  },
  conform_integration = true,
}

-- Validate configuration
function M.validate(config)
  local errors = {}

  if type(config.cmd) ~= "string" then
    table.insert(errors, "cmd must be a string")
  end

  if type(config.autostart) ~= "boolean" then
    table.insert(errors, "autostart must be a boolean")
  end

  if type(config.format_on_save) ~= "boolean" then
    table.insert(errors, "format_on_save must be a boolean")
  end

  if type(config.filetypes) ~= "table" then
    table.insert(errors, "filetypes must be a table")
  else
    for i, ft in ipairs(config.filetypes) do
      if type(ft) ~= "string" then
        table.insert(errors, "filetypes[" .. i .. "] must be a string")
      end
    end
  end

  if config.keymaps ~= nil and type(config.keymaps) ~= "table" then
    table.insert(errors, "keymaps must be a table or nil")
  end

  if type(config.conform_integration) ~= "boolean" then
    table.insert(errors, "conform_integration must be a boolean")
  end

  return #errors == 0, errors
end

-- Merge user config with defaults
function M.merge(user_config)
  local config = vim.tbl_deep_extend("force", M.defaults, user_config or {})

  local is_valid, errors = M.validate(config)
  if not is_valid then
    error("Invalid snapper configuration: " .. table.concat(errors, ", "))
  end

  return config
end

return M