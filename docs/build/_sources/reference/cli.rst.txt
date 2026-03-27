

CLI Reference
-------------

Synopsis
~~~~~~~~

.. code:: text

    snapper [OPTIONS] [FILE...]

If no files are given, ``snapper`` reads from stdin and writes to stdout.

Options
~~~~~~~

``-f``, ``--format <FORMAT>``
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Input format.
One of: ``org``, ``latex``, ``markdown``, ``plaintext``.

Auto-detected from file extension when omitted.
Defaults to ``plaintext`` for stdin.

``-o``, ``--output <FILE>``
^^^^^^^^^^^^^^^^^^^^^^^^^^^

Write output to a file instead of stdout.

``-i``, ``--in-place``
^^^^^^^^^^^^^^^^^^^^^^

Modify input files in place.
Requires file arguments (not stdin).

``-w``, ``--max-width <N>``
^^^^^^^^^^^^^^^^^^^^^^^^^^^

Maximum line width.
Sentences exceeding this width are wrapped at word boundaries using ``textwrap``.
Default: ``0`` (unlimited).

``--check``
^^^^^^^^^^^

Check mode for CI.
Exits with code 1 if any file would change, without modifying anything.
Prints the paths of files that would be reformatted to stderr.

``--diff``
^^^^^^^^^^

Show a unified diff of what would change, without modifying anything.
Useful for reviewing before committing.
Exits with code 1 if any file would change.

``--config <PATH>``
^^^^^^^^^^^^^^^^^^^

Path to a ``.snapperrc.toml`` config file.
When omitted, ``snapper`` searches from the current directory upward.

``--neural``
^^^^^^^^^^^^

Use neural sentence detection via ``nnsplit``.
Requires building with ``--features neural``.
Useful for non-English text where rule-based splitting produces poor results.

``-h``, ``--help``
^^^^^^^^^^^^^^^^^^

Print help message.

``-V``, ``--version``
^^^^^^^^^^^^^^^^^^^^^

Print version.

Exit codes
~~~~~~~~~~

- ``0`` : Success (or ``--check`` / ``--diff`` with no changes needed)

- ``1`` : ``--check`` or ``--diff`` found files that would change, or any error
