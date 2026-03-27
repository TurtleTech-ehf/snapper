

Quickstart
----------

Installation
~~~~~~~~~~~~

From `crates.io <https://crates.io/crates/snapper-fmt>`_ (the crate is ``snapper-fmt``, the binary it installs is ``snapper``):

.. code:: bash

    cargo install snapper-fmt

Or build from source:

.. code:: bash

    cargo build --release
    # Binary at ./target/release/snapper

With Nix:

.. code:: bash

    nix build github:TurtleTech-ehf/snapper

Basic usage
~~~~~~~~~~~

Format a file, printing to stdout:

.. code:: bash

    snapper paper.org

Format in place:

.. code:: bash

    snapper --in-place paper.org

Pipe through stdin (for editor integration):

.. code:: bash

    cat draft.org | snapper --format org

What it does
~~~~~~~~~~~~

Given a paragraph like this:

.. code:: text

    This is the first sentence. It continues with more details about the topic. See Fig. 3 for the results.

``snapper`` produces:

.. code:: text

    This is the first sentence.
    It continues with more details about the topic.
    See Fig. 3 for the results.

Each sentence occupies its own line.
Structural elements like code blocks, tables, drawers, math environments, and front matter pass through unchanged.

Format detection
~~~~~~~~~~~~~~~~

``snapper`` auto-detects the format from the file extension:

- ``.org`` -- Org-mode

- ``.tex``, ``.latex`` -- LaTeX

- ``.md``, ``.markdown`` -- Markdown

- Everything else -- plaintext

Override with ``--format``:

.. code:: bash

    snapper --format latex draft.tex

CI integration
~~~~~~~~~~~~~~

Use ``--check`` mode to verify formatting without modifying files.
Exits with code 1 if any file would change:

.. code:: bash

    snapper --check paper.org

Pre-commit hook
~~~~~~~~~~~~~~~

Add to your ``.pre-commit-config.yaml``:

.. code:: yaml

    - repo: https://github.com/TurtleTech-ehf/snapper
      rev: v0.1.0
      hooks:
        - id: snapper

Emacs (Apheleia)
~~~~~~~~~~~~~~~~

Add to your Emacs config:

.. code:: elisp

    (with-eval-after-load 'apheleia
      (push '(snapper . ("snapper" "--format" "org")) apheleia-formatters)
      (push '(org-mode . snapper) apheleia-mode-alist))

This runs ``snapper`` on save for Org-mode buffers.
