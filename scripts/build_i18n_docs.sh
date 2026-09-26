#!/usr/bin/env bash
# Build Sphinx docs for all languages (en, is, pl).
# Outputs: docs/build/ (en), docs/build/is/, docs/build/pl/
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# 1. Export org -> RST
emacs --batch -l docs/export.el

# 2. Extract translatable strings
sphinx-build -b gettext docs/source docs/locale/pot

# 3. Update .po files for each language
for lang in is pl; do
    sphinx-intl update -p docs/locale/pot -l "$lang" -d docs/locale
done

# sphinx-intl skips a rewrite when msgids are unchanged, leaving stale
# POT-Creation-Date headers. msgmerge copies the date from the current POT.
if command -v msgmerge >/dev/null 2>&1; then
    while IFS= read -r -d '' po; do
        rel="${po#docs/locale/}"
        rest="${rel#*/LC_MESSAGES/}"
        pot="docs/locale/pot/${rest%.po}.pot"
        if [[ -f "$pot" ]]; then
            msgmerge --update --backup=none "$po" "$pot"
        fi
    done < <(find docs/locale -name '*.po' -print0)
fi

# 4. Build English (default)
sphinx-build -b html docs/source docs/build

# 5. Build each translation
for lang in is pl; do
    SPHINX_LANGUAGE="$lang" sphinx-build -b html docs/source "docs/build/$lang" \
        -D "language=$lang"
done

echo "Docs built: en (docs/build/), is (docs/build/is/), pl (docs/build/pl/)"
