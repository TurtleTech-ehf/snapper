=======================
Git Smudge/Clean Filter
=======================



Overview
--------

A git smudge/clean filter auto-formats files on commit (clean) and optionally restores the original wrapping on checkout (smudge).
This makes semantic line breaks transparent to collaborators who do not use snapper.

Setup
-----

Configure the filter in your local git config:

.. code:: bash

    git config filter.snapper.clean "snapper --format org"
    git config filter.snapper.smudge cat

The ``smudge`` filter uses ``cat`` (passthrough), meaning checked-out files retain semantic line breaks.
If you want to restore traditional wrapping on checkout, replace ``cat`` with a rewrapping command.

Activate via .gitattributes
---------------------------

Add to your repository's ``.gitattributes``:

.. code:: text

    *.org filter=snapper
    *.tex filter=snapper
    *.md  filter=snapper

Commit ``.gitattributes`` to share with collaborators.
The filter only activates for people who have configured it locally.

Per-format filters
------------------

If you need different settings per format:

.. code:: bash

    git config filter.snapper-org.clean "snapper --format org"
    git config filter.snapper-org.smudge cat

    git config filter.snapper-tex.clean "snapper --format latex"
    git config filter.snapper-tex.smudge cat

.. code:: text

    *.org filter=snapper-org
    *.tex filter=snapper-tex

Verifying the filter works
--------------------------

After setting up, stage a file and check the diff:

.. code:: bash

    git diff --cached paper.org

The staged version should show sentence-per-line formatting even if the working copy uses traditional wrapping.
