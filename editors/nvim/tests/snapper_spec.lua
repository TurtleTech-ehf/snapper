-- Tests for snapper.nvim plugin
local snapper = require('snapper')

describe('snapper', function()
  local original_vim

  before_each(function()
    original_vim = vim

    vim = {
      fn = {
        executable = function(cmd)
          return cmd == "snapper" and 1 or 0
        end,
        has = function(feature)
          return feature == "nvim-0.10" and 1 or 0
        end,
        system = function(cmd)
          return ""
        end,
      },
      v = {
        shell_error = 0,
      },
      api = {
        nvim_create_user_command = function(name, callback, opts)
        end,
        nvim_create_autocmd = function(event, opts)
        end,
        nvim_echo = function(messages, history, opts)
        end,
        nvim_buf_get_name = function(bufnr)
          return "/tmp/test.org"
        end,
        nvim_get_current_buf = function()
          return 1
        end,
      },
      cmd = function(str)
      end,
      notify = function(msg, level)
      end,
      tbl_deep_extend = function(strategy, ...)
        local result = {}
        for _, tbl in ipairs({...}) do
          for k, v in pairs(tbl) do
            result[k] = v
          end
        end
        return result
      end,
      bo = {},
      g = {},
      lsp = {
        buf = {
          format = function(opts)
          end,
        },
        buf_request_sync = function(buf, method, params, timeout)
          return {}
        end,
        util = {
          make_format_params = function(opts)
            return {}
          end,
          apply_text_edits = function(edits, buf, encoding)
          end,
        },
        start = function(opts)
          return { id = 1 }
        end,
        get_clients = function(opts)
          return {}
        end,
        protocol = {
          make_client_capabilities = function()
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
      log = {
        levels = {
          INFO = 2,
          WARN = 3,
          ERROR = 4,
        },
      },
    }

    snapper.config = {}
  end)

  after_each(function()
    vim = original_vim
  end)

  it('should have default configuration', function()
    assert.are.same({}, snapper.config)
  end)

  it('should setup with default options', function()
    snapper.setup({})

    assert.is.truthy(snapper.config.cmd)
    assert.equal("snapper", snapper.config.cmd)
    assert.is.truthy(snapper.config.filetypes)
    assert.are.same({ "org", "tex", "markdown", "rst", "plaintext" }, snapper.config.filetypes)
  end)

  it('should override default configuration', function()
    snapper.setup({
      cmd = "custom-snapper",
      format_on_save = true,
      filetypes = { "org" }
    })

    assert.equal("custom-snapper", snapper.config.cmd)
    assert.is_true(snapper.config.format_on_save)
    assert.are.same({ "org" }, snapper.config.filetypes)
  end)

  it('should warn when snapper binary not found', function()
    vim.fn.executable = function(cmd) return 0 end

    local notify_called = false
    vim.notify = function(msg, level)
      notify_called = true
    end

    snapper.setup({ cmd = "nonexistent" })
    assert.is_true(notify_called)
  end)

  it('should define formatexpr function', function()
    assert.is_function(snapper.formatexpr)
  end)

  it('should create commands', function()
    local commands_created = {}
    vim.api.nvim_create_user_command = function(name, callback, opts)
      commands_created[name] = { callback = callback, opts = opts }
    end

    snapper.setup({})

    assert.truthy(commands_created.SnapperFormat)
    assert.truthy(commands_created.SnapperFormatRange)
    assert.truthy(commands_created.SnapperCheck)
    assert.truthy(commands_created.SnapperRestart)
    assert.truthy(commands_created.SnapperInfo)
  end)

  it('should show info', function()
    snapper.setup({})

    local echo_called = false
    vim.api.nvim_echo = function(messages, history, opts)
      echo_called = true
      local msg = messages[1][1]
      assert.truthy(string.find(msg, "Snapper Info"))
      assert.truthy(string.find(msg, "Binary: snapper"))
    end

    snapper.show_info()
    assert.is_true(echo_called)
  end)
end)
