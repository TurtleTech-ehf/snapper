local M = {}

function M.setup(config)
  local augroup = vim.api.nvim_create_augroup("SnapperLSP", { clear = true })

  vim.api.nvim_create_autocmd("FileType", {
    group = augroup,
    pattern = config.filetypes,
    callback = function(args)
      M.start(config, args.buf)
    end,
  })
end

function M.start(config, bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()

  -- vim.lsp.start() deduplicates by name+root_dir, so no manual tracking needed
  vim.lsp.start({
    name = "snapper",
    cmd = { config.cmd, "lsp" },
    root_dir = vim.fs.root(bufnr, { ".snapperrc.toml", ".git" })
      or vim.fs.dirname(vim.api.nvim_buf_get_name(bufnr)),
    on_attach = function(client, attached_bufnr)
      M.on_attach(client, attached_bufnr, config)
    end,
    capabilities = vim.lsp.protocol.make_client_capabilities(),
  })
end

function M.on_attach(client, bufnr, config)
  vim.bo[bufnr].formatexpr = "v:lua.require('snapper').formatexpr()"

  if config.keymaps then
    local opts = { buffer = bufnr, silent = true }
    if config.keymaps.format then
      vim.keymap.set("n", config.keymaps.format, function()
        vim.lsp.buf.format({ name = "snapper" })
      end, opts)
    end
    if config.keymaps.format_range then
      vim.keymap.set("v", config.keymaps.format_range, function()
        vim.lsp.buf.format({ name = "snapper" })
      end, opts)
    end
    if config.keymaps.check then
      vim.keymap.set("n", config.keymaps.check, function()
        vim.api.nvim_command("SnapperCheck")
      end, opts)
    end
  end

  if config.format_on_save then
    vim.api.nvim_create_autocmd("BufWritePre", {
      buffer = bufnr,
      callback = function()
        vim.lsp.buf.format({
          name = "snapper",
          async = config.format_on_save_opts.async,
          timeout_ms = config.format_on_save_opts.timeout_ms,
        })
      end,
    })
  end
end

function M.restart()
  local clients = vim.lsp.get_clients({ name = "snapper" })
  for _, client in ipairs(clients) do
    client:stop(true)
  end
  -- Will auto-restart on next FileType event
end

function M.is_running()
  return #vim.lsp.get_clients({ name = "snapper" }) > 0
end

return M
