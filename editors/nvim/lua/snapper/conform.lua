local M = {}

function M.setup(config)
  local conform_avail, conform = pcall(require, "conform")
  if not conform_avail then
    return
  end

  -- Register formatter
  conform.formatters.snapper = {
    command = config.cmd,
    args = { "--stdin-filepath", "$FILENAME" },
    stdin = true,
    cwd = require("conform.util").root_file({ ".snapperrc.toml" }),
  }

end

return M