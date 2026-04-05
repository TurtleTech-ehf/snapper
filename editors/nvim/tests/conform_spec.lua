-- Tests for snapper.conform module
local conform_module = require('snapper.conform')

describe('snapper.conform', function()
  local original_vim

  before_each(function()
    original_vim = vim

    vim = {
      fn = {
        executable = function(cmd)
          return cmd == "snapper" and 1 or 0
        end,
      },
      api = {
        nvim_create_user_command = function(name, callback, opts)
        end,
      },
    }
  end)

  after_each(function()
    vim = original_vim
  end)

  it('should setup conform integration when conform is available', function()
    local mock_conform = {
      formatters = {},
    }

    local original_require = require
    _G.require = function(module_name)
      if module_name == "conform" then
        return mock_conform
      elseif module_name == "conform.util" then
        return {
          root_file = function(markers)
            return "/tmp"
          end
        }
      else
        return original_require(module_name)
      end
    end

    local original_pcall = pcall
    _G.pcall = function(fn, ...)
      if select(1, ...) == "conform" then
        return true, mock_conform
      else
        return original_pcall(fn, ...)
      end
    end

    local config = {
      cmd = "snapper",
    }

    local success, err = pcall(conform_module.setup, config)
    assert.is_true(success, "Setup should succeed: " .. tostring(err))

    assert.truthy(mock_conform.formatters.snapper)
    assert.equal("snapper", mock_conform.formatters.snapper.command)

    _G.require = original_require
    _G.pcall = original_pcall
  end)

  it('should handle case when conform is not available', function()
    local original_require = require
    _G.require = function(module_name)
      if module_name == "conform" then
        error("module 'conform' not found")
      else
        return original_require(module_name)
      end
    end

    local original_pcall = pcall
    _G.pcall = function(fn, ...)
      if select(1, ...) == "conform" then
        return false, nil
      else
        return original_pcall(fn, ...)
      end
    end

    local config = {
      cmd = "snapper",
    }

    local success, err = pcall(conform_module.setup, config)
    assert.is_true(success, "Setup should succeed even when conform is not available: " .. tostring(err))

    _G.require = original_require
    _G.pcall = original_pcall
  end)

  it('should set correct formatter configuration', function()
    local mock_conform = {
      formatters = {},
    }

    local original_require = require
    _G.require = function(module_name)
      if module_name == "conform" then
        return mock_conform
      elseif module_name == "conform.util" then
        return {
          root_file = function(markers)
            return "/tmp"
          end
        }
      else
        return original_require(module_name)
      end
    end

    local original_pcall = pcall
    _G.pcall = function(fn, ...)
      if select(1, ...) == "conform" then
        return true, mock_conform
      else
        return original_pcall(fn, ...)
      end
    end

    local config = {
      cmd = "custom-snapper-path",
    }

    conform_module.setup(config)

    local formatter = mock_conform.formatters.snapper
    assert.equal("custom-snapper-path", formatter.command)
    assert.same({ "--stdin-filepath", "$FILENAME" }, formatter.args)
    assert.is_true(formatter.stdin)

    _G.require = original_require
    _G.pcall = original_pcall
  end)
end)
