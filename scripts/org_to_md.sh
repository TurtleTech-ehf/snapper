#!/usr/bin/env bash
# Convert org-mode to GitHub-flavored Markdown via Emacs batch
# Usage: ./scripts/org_to_md.sh <input.org> [output.md]
set -euo pipefail

INPUT="${1:?Usage: org_to_md.sh <input.org> [output.md]}"
OUTPUT="${2:-${INPUT%.org}.md}"

# Emacs needs the file to have .org extension
TMPORG=$(mktemp --suffix=.org)
TMPMD="${TMPORG%.org}.md"
cp "$INPUT" "$TMPORG"

emacs --batch \
  --eval "(require 'ox-md)" \
  --eval "(find-file \"$TMPORG\")" \
  --eval "(org-md-export-to-markdown)" \
  --kill 2>/dev/null

# ox-md rewrites file: links to .md even when only .org exists in-tree.
# Prefer keeping org paths for docs that are not published as markdown.
if [[ -f "$TMPMD" ]]; then
  sed -i -E \
    -e 's|(docs/orgmode/[^)]+)\.md|\1.org|g' \
    "$TMPMD"
fi

mv "$TMPMD" "$OUTPUT"
rm -f "$TMPORG"
echo "Wrote $OUTPUT"
