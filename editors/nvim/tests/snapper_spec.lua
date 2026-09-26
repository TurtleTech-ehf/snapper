-- Tests for snapper.nvim plugin

describe('snapper', function()
  local snapper

  before_each(function()
    -- Clear cached modules so each test starts fresh
    package.loaded['snapper'] = nil
    package.loaded['snapper.config'] = nil
    package.loaded['snapper.lsp'] = nil
    package.loaded['snapper.conform'] = nil
    snapper = require('snapper')
  end)

  it('should have default configuration', function()
    assert.are.same({}, snapper.config)
  end)

  it('should define formatexpr function', function()
    assert.is_function(snapper.formatexpr)
  end)

  describe('config', function()
    local config_module

    before_each(function()
      package.loaded['snapper.config'] = nil
      config_module = require('snapper.config')
    end)

    it('should have sensible defaults', function()
      local defaults = config_module.defaults
      assert.equal("snapper", defaults.cmd)
      assert.is_true(defaults.autostart)
      assert.is_false(defaults.format_on_save)
      assert.is_true(defaults.conform_integration)
      assert.are.same({ "org", "tex", "markdown", "rst", "plaintext" }, defaults.filetypes)
    end)

    it('should merge user config with defaults', function()
      local merged = config_module.merge({
        cmd = "custom-snapper",
        format_on_save = true,
      })
      assert.equal("custom-snapper", merged.cmd)
      assert.is_true(merged.format_on_save)
      assert.is_true(merged.autostart) -- default preserved
    end)

    it('should validate config types', function()
      local ok, errors = config_module.validate({
        cmd = 123, -- should be string
        autostart = true,
        format_on_save = false,
        filetypes = { "org" },
        keymaps = nil,
        conform_integration = true,
        format_on_save_opts = {},
      })
      assert.is_false(ok)
      assert.truthy(#errors > 0)
    end)

    it('should accept valid config', function()
      local ok, errors = config_module.validate(config_module.defaults)
      assert.is_true(ok)
      assert.are.same({}, errors)
    end)

    it('should reject invalid filetypes', function()
      local ok, errors = config_module.validate(vim.tbl_deep_extend("force", config_module.defaults, {
        filetypes = { 123 },
      }))
      assert.is_false(ok)
    end)
  end)
end)
