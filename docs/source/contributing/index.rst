============
Contributing
============

    :Author: Rohit Goswami


Development setup
-----------------

.. code:: bash

    git clone https://github.com/TurtleTech-ehf/snapper
    cd snapper
    cargo build
    cargo test

With pixi:

.. code:: bash

    pixi run -e dev check

Running checks
--------------

.. code:: bash

    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test

Dogfood check (verify all docs pass formatting):

.. code:: bash

    pixi run -e dev dogfood

Project structure
-----------------

::

    src/
      main.rs             -- Entry point, CLI dispatch
      lib.rs              -- Public API: format_text(), FormatConfig
      cli.rs              -- Clap derive CLI definition
      config.rs           -- .snapperrc.toml project config loader
      format.rs           -- Format enum + auto-detection from extension
      diff.rs             -- Unified diff output for --diff mode
      abbreviations.rs    -- Built-in abbreviation lists
      reflow.rs           -- Core reflow engine (regions + splitter -> output)
      sentence/
        mod.rs            -- SentenceSplitter trait
        unicode.rs        -- UAX #29 + abbreviation merge + placeholder system
      parser/
        mod.rs            -- Region enum, FormatParser trait
        org.rs            -- Org-mode parser
        latex.rs          -- LaTeX parser
        markdown.rs       -- Markdown parser
        plaintext.rs      -- Plaintext parser (everything is prose)
    tests/
      integration.rs      -- Integration tests (format, check, idempotency, stdin)
      fixtures/           -- Sample input/expected output pairs per format
    vale/
      snapper/            -- Vale style rules for editor hints
    site/                 -- Landing page (static HTML)
    docs/
      orgmode/            -- Documentation source (org-mode)
      source/             -- Sphinx config + templates
      export.el           -- Batch org -> RST exporter

Commit conventions
------------------

Commits follow conventional commits, managed by ``cocogitto`` (``cog``):

``feat:``
    new features

``fix:``
    bug fixes

``doc:``
    documentation changes

``chore:``
    maintenance, dependency bumps

``tst:``
    test changes

``bld:``
    build system changes

Building documentation
----------------------

.. code:: bash

    pixi run -e docs docbld

Or manually:

.. code:: bash

    emacs --batch -l docs/export.el
    sphinx-build docs/source docs/build -b html

Adding a new format parser
--------------------------

1. Create ``src/parser/yourformat.rs`` implementing the ``FormatParser`` trait

2. Add the variant to ``Format`` enum in ``src/format.rs``

3. Wire it into ``format_text()`` in ``src/lib.rs``

4. Add extension mapping in ``Format::from_path()``

5. Add fixture pair in ``tests/fixtures/`` (``sample.fmt`` + ``expected.fmt``)

6. Add integration tests in ``tests/integration.rs``

7. Document in ``docs/orgmode/reference/formats.org``
