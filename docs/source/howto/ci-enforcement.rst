==============
CI Enforcement
==============



Pre-commit hook
---------------

The simplest CI integration.
Add to ``.pre-commit-config.yaml``:

.. code:: yaml

    - repo: https://github.com/TurtleTech-ehf/snapper
      rev: v0.1.0
      hooks:
        - id: snapper

This runs ``snapper --in-place`` on changed files matching ``.org``, ``.tex``, ``.md``, and ``.txt``.

GitHub Actions
--------------

Add a formatting check step to your CI workflow:

.. code:: yaml

    - name: Check semantic line breaks
      run: |
        cargo install snapper-fmt
        snapper --check paper.org sections/*.org

The ``--check`` flag exits with code 1 if any file needs reformatting, without modifying anything.

Preview changes in CI
---------------------

Use ``--diff`` to show what would change:

.. code:: yaml

    - name: Show formatting diff
      run: snapper --diff paper.org sections/*.org

This prints a unified diff to stdout, useful for PR review comments.

GitLab CI
---------

.. code:: yaml

    format-check:
      image: rust:latest
      script:
        - cargo install snapper-fmt
        - snapper --check **/*.org **/*.tex **/*.md
      allow_failure: false

Makefile integration
--------------------

.. code:: makefile

    .PHONY: fmt fmt-check

    fmt:
            snapper --in-place paper.org sections/*.org

    fmt-check:
            snapper --check paper.org sections/*.org
