-- Tests for snapper.conform module

describe('snapper.conform', function()
  local conform_module

  before_each(function()
    package.loaded['snapper.conform'] = nil
    conform_module = require('snapper.conform')
  end)

  it('should export setup function', function()
    assert.is_function(conform_module.setup)
  end)

  it('should handle case when conform is not installed', function()
    -- conform.nvim is not installed in the test environment
    -- setup should return without error
    local config = { cmd = "snapper" }
    assert.has_no.errors(function()
      conform_module.setup(config)
    end)
  end)
end)
