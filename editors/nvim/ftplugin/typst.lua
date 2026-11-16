vim.bo.formatprg = "snapper --native --format typst"

local ok, _ = pcall(require, "snapper")
if ok then
  vim.bo.formatexpr = "v:lua.require('snapper').formatexpr()"
end
