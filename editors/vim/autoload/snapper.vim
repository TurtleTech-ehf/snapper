" snapper.vim autoload functions

" Check if snapper binary is available
function! snapper#available()
  return executable('snapper')
endfunction

" Check formatting
function! snapper#check()
  if !snapper#available()
    echo 'snapper binary not found'
    return
  endif

  let l:output = system('snapper --check ' . shellescape(expand('%')))
  if v:shell_error == 0
    echo 'Snapper: file is already formatted'
  else
    echohl WarningMsg
    echo 'Snapper: file needs formatting'
    echohl None
    echo l:output
  endif
endfunction

" Restart functionality
function! snapper#restart()
  " For Vim compatibility, this is a placeholder
  " since Vim doesn't have LSP client management like Neovim
  if !snapper#available()
    echo 'snapper binary not found'
    return
  endif
  
  echo 'Snapper: Restart functionality not applicable in Vim'
endfunction

" Info functionality
function! snapper#info()
  if !snapper#available()
    echo 'snapper binary not found'
    return
  endif
  
  let version_output = system('snapper --version 2>&1')
  echo 'Snapper Info:'
  echo '  Binary: available'
  echo '  Version: ' . trim(version_output)
  echo '  Supported formats: org, tex, markdown, rst, plaintex'
endfunction