-- Minimal init.lua for testing
vim.opt.runtimepath:append(vim.fn.getcwd())
vim.opt.runtimepath:append(vim.fn.stdpath('test') .. '/fixtures/runtime')

-- Load the snapper plugin
local snapper_path = vim.fn.fnamemodify(debug.getinfo(1).source:match("@?(.*)", 1), ":p:h:h") .. "/lua"
package.path = package.path .. ";" .. snapper_path .. "/?.lua;" .. snapper_path .. "/?/init.lua"

-- Mock required functions if not available in test environment
if not vim.ui then
  vim.ui = {}
end

if not vim.ui.select then
  vim.ui.select = function(items, opts, callback)
    callback(items[1], 1)
  end
end