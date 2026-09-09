" snapper.vim - Vim plugin for snapper semantic line break formatter
" Maintainer: TurtleTech
" Version: 1.0.0

if exists('g:loaded_snapper')
  finish
endif
let g:loaded_snapper = 1

" Commands
command! -range=% SnapperFormat <line1>,<line2>!snapper --native --stdin-filepath %:S
command! SnapperCheck call snapper#check()
command! SnapperRestart call snapper#restart()
command! SnapperInfo call snapper#info()

" Auto-detect filetype for formatprg
augroup snapper
  autocmd!
  autocmd FileType org setlocal formatprg=snapper\ --native\ --format\ org
  autocmd FileType tex setlocal formatprg=snapper\ --native\ --format\ latex
  autocmd FileType markdown setlocal formatprg=snapper\ --native\ --format\ markdown
  autocmd FileType rst setlocal formatprg=snapper\ --native\ --format\ rst
  autocmd FileType plaintex setlocal formatprg=snapper\ --native\ --format\ plaintex
augroup END