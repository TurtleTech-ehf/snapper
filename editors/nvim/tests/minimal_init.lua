-- Minimal init.lua for testing
vim.opt.runtimepath:append(vim.fn.getcwd())

-- Load the snapper plugin
local snapper_path = vim.fn.fnamemodify(debug.getinfo(1).source:match("@?(.*)", 1), ":p:h:h")
vim.opt.runtimepath:append(snapper_path)

-- Add lua path for require
local lua_path = snapper_path .. "/lua"
package.path = package.path .. ";" .. lua_path .. "/?.lua;" .. lua_path .. "/?/init.lua"
