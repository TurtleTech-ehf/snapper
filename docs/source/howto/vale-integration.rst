================
Vale Integration
================



Overview
--------

Snapper ships a vale style package with two rules that flag lines needing semantic line breaks.
For precise enforcement, use ``snapper --check`` directly in CI.

Style package setup
-------------------

Add the snapper vale style to your ``.vale.ini``:

.. code:: ini

    StylesPath = /path/to/snapper/vale
    MinAlertLevel = suggestion

    [*.org]
    BasedOnStyles = snapper

    [*.tex]
    BasedOnStyles = snapper

    [*.md]
    BasedOnStyles = snapper

Bundled rules
-------------

``SemanticLineBreaks``
    Flags lines containing multiple sentences (a period followed by a space and a capital letter on the same line).

Level: warning.

``LongProseLine``
    Flags prose lines exceeding 120 characters.

Level: suggestion.

Precise CI enforcement
----------------------

The vale rules use regex heuristics.
For accurate checking, use the formatter directly:

.. code:: bash

    snapper --check paper.org

This catches all cases the regex rules miss (abbreviations, inline tokens, etc.).

Combining vale and snapper
--------------------------

A typical workflow:

1. vale catches style issues (AI tells, hedging, E-Prime) during editing

2. snapper enforces line break formatting in CI via ``--check``

3. Pre-commit hook runs both:

.. code:: yaml

    - repo: https://github.com/TurtleTech-ehf/snapper
      rev: v0.1.0
      hooks:
        - id: snapper
    - repo: https://github.com/errata-ai/vale
      rev: v3.0.0
      hooks:
        - id: vale
