==================
Editor Integration
==================



Emacs (Apheleia)
----------------

`Apheleia <https://github.com/radian-software/apheleia>`_ runs external formatters on buffer save, preserving cursor position.

.. code:: elisp

    (with-eval-after-load 'apheleia
      (push '(snapper . ("snapper" "--format" "org")) apheleia-formatters)
      (push '(org-mode . snapper) apheleia-mode-alist)
      ;; Add for other formats:
      (push '(latex-mode . snapper) apheleia-mode-alist)
      (push '(markdown-mode . snapper) apheleia-mode-alist))

The ``--format`` flag auto-detects from the file extension, so you can omit it if your files have standard extensions.

Vim / Neovim
------------

Use ``formatprg`` to pipe through snapper:

.. code:: vim

    autocmd FileType org setlocal formatprg=snapper\ --format\ org
    autocmd FileType tex setlocal formatprg=snapper\ --format\ latex
    autocmd FileType markdown setlocal formatprg=snapper\ --format\ markdown

Then ``gq`` to reformat selected text, or ``gggqG`` to reformat the entire buffer.

For Neovim with ``conform.nvim``:

.. code:: lua

    require("conform").setup({
      formatters = {
        snapper = {
          command = "snapper",
          args = { "--format", "$FILETYPE" },
          stdin = true,
        },
      },
      formatters_by_ft = {
        org = { "snapper" },
        tex = { "snapper" },
        markdown = { "snapper" },
      },
    })

VS Code
-------

Use the "Run on Save" extension with a custom command:

.. code:: json

    {
      "emeraldwalk.runonsave": {
        "commands": [
          {
            "match": "\\.(org|tex|md)$",
            "cmd": "snapper --in-place ${file}"
          }
        ]
      }
    }

Generic (any editor)
--------------------

Snapper reads stdin and writes stdout by default.
Any editor that supports piping through an external program works:

.. code:: bash

    cat paper.org | snapper --format org
