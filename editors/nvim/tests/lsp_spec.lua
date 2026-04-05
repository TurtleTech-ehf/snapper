-- Tests for snapper.lsp module

describe('snapper.lsp', function()
  local lsp_module

  before_each(function()
    package.loaded['snapper.lsp'] = nil
    lsp_module = require('snapper.lsp')
  end)

  it('should export setup function', function()
    assert.is_function(lsp_module.setup)
  end)

  it('should export start function', function()
    assert.is_function(lsp_module.start)
  end)

  it('should export on_attach function', function()
    assert.is_function(lsp_module.on_attach)
  end)

  it('should export restart function', function()
    assert.is_function(lsp_module.restart)
  end)

  it('should export is_running function', function()
    assert.is_function(lsp_module.is_running)
  end)

  it('should report not running when no LSP clients exist', function()
    -- No snapper LSP client is started in tests
    assert.is_false(lsp_module.is_running())
  end)

  it('should not error when restarting with no clients', function()
    assert.has_no.errors(function()
      lsp_module.restart()
    end)
  end)

  it('should setup autocmd group without errors', function()
    local config = {
      cmd = "snapper",
      filetypes = { "org", "markdown" },
      format_on_save = false,
      keymaps = { format = "<leader>sf" },
    }
    assert.has_no.errors(function()
      lsp_module.setup(config)
    end)
  end)
end)
