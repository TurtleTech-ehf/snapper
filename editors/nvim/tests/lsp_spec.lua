-- Tests for snapper.lsp module
local lsp_module = require('snapper.lsp')

describe('snapper.lsp', function()
  local original_vim

  before_each(function()
    original_vim = vim

    vim = {
      api = {
        nvim_create_augroup = function(name, opts)
          return 1
        end,
        nvim_create_autocmd = function(event, opts)
        end,
        nvim_get_current_buf = function()
          return 1
        end,
      },
      fn = {
        executable = function(cmd)
          return cmd == "snapper" and 1 or 0
        end,
      },
      lsp = {
        start = function(opts)
          return { id = 1 }
        end,
        get_clients = function(opts)
          if opts and opts.name == "snapper" then
            return {}
          end
          return {}
        end,
        buf = {
          format = function(opts)
          end,
        },
        protocol = {
          make_client_capabilities = function()
            return {}
          end,
        },
        util = {
          make_given_range_params = function()
            return {}
          end,
        },
      },
      fs = {
        root = function(buf, markers)
          return "/tmp"
        end,
        dirname = function(path)
          return "/tmp"
        end,
      },
      bo = {
        [1] = {},
      },
      keymap = {
        set = function(mode, lhs, rhs, opts)
        end,
      },
    }
  end)

  after_each(function()
    vim = original_vim
  end)

  it('should setup LSP for filetypes', function()
    local config = {
      cmd = "snapper",
      filetypes = { "org", "markdown" },
      format_on_save = false,
      keymaps = { format = "<leader>sf" },
    }

    local success, err = pcall(lsp_module.setup, config)
    assert.is_true(success, "Setup should succeed: " .. tostring(err))
  end)

  it('should start LSP client', function()
    local config = {
      cmd = "snapper",
      filetypes = { "org" },
    }

    local success, err = pcall(lsp_module.start, config, 1)
    assert.is_true(success, "Start should succeed: " .. tostring(err))
  end)

  it('should handle on_attach properly', function()
    local config = {
      cmd = "snapper",
      format_on_save = false,
      keymaps = { format = "<leader>sf" },
    }

    local mock_client = { id = 1 }
    local success, err = pcall(lsp_module.on_attach, mock_client, 1, config)
    assert.is_true(success, "on_attach should succeed: " .. tostring(err))
  end)

  it('should handle format on save', function()
    local config = {
      cmd = "snapper",
      format_on_save = true,
      format_on_save_opts = {
        timeout_ms = 1000,
        async = false,
      },
      keymaps = nil,
    }

    local autocmd_set = false
    local original_create_autocmd = vim.api.nvim_create_autocmd
    vim.api.nvim_create_autocmd = function(event, opts)
      if event == "BufWritePre" then
        autocmd_set = true
      end
    end

    local mock_client = { id = 1 }
    lsp_module.on_attach(mock_client, 1, config)

    assert.is_true(autocmd_set, "BufWritePre autocmd should be set")

    vim.api.nvim_create_autocmd = original_create_autocmd
  end)

  it('should restart LSP clients', function()
    local success, err = pcall(lsp_module.restart)
    assert.is_true(success, "Restart should succeed: " .. tostring(err))
  end)

  it('should report LSP not running when no clients exist', function()
    assert.is_false(lsp_module.is_running())
  end)

  it('should report LSP running when clients exist', function()
    local original_get_clients = vim.lsp.get_clients
    vim.lsp.get_clients = function(opts)
      if opts and opts.name == "snapper" then
        return {{ id = 1 }}
      end
      return {}
    end

    assert.is_true(lsp_module.is_running())

    vim.lsp.get_clients = original_get_clients
  end)
end)
