#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
public_sources=(
  "$repo_root/readme_src.org"
  "$repo_root/README.md"
  "$repo_root/npm/README.md"
  "$repo_root/docs/orgmode/tutorials/quickstart.org"
  "$repo_root/docs/orgmode/howto/editor-integration.org"
  "$repo_root/docs/orgmode/howto/mcp-integration.org"
  "$repo_root/editors/obsidian/README.md"
  "$repo_root/editors/word/README.md"
)

required_patterns=(
  "cargo install snapper-fmt --features mcp"
  "snapper mcp"
  "marketplace.visualstudio.com/items?itemName=TurtleTech.snapper"
  "Development preview; not listed in Community Plugins"
  "Development preview; not published in AppSource"
)

unsupported_patterns=(
  "npx @turtletech/snapper-mcp"
  "@turtletech/snapper-mcp"
  '"command": "npx"'
  "Install the snapper plugin from Community Plugins"
  "Install from Community Plugins"
  "Install the snapper add-in from AppSource"
  "In Word: Insert > Get Add-ins > search \"Snapper\""
  "Download =main.js= and =manifest.json= from"
  "Download the add-in ZIP from"
)

failed=0

if ! grep -Fq '"private": true' "$repo_root/npm/package.json"; then
  printf 'unpublished npm wrapper must be private\n' >&2
  failed=1
fi

if grep -Eq '\|\|[[:space:]]+true' "$repo_root/scripts/build_wasm_editors.sh"; then
  printf 'editor build helper must propagate every build failure\n' >&2
  failed=1
fi

if grep -Eq 'npm[[:space:]]+install' "$repo_root/scripts/build_wasm_editors.sh"; then
  printf 'editor build helper must use locked npm installs\n' >&2
  failed=1
fi

for pattern in "${required_patterns[@]}"; do
  if ! grep -Fq "$pattern" "${public_sources[@]}"; then
    printf 'public docs are missing required content: %s\n' "$pattern" >&2
    failed=1
  fi
done

obsidian_sources=(
  "$repo_root/readme_src.org"
  "$repo_root/README.md"
  "$repo_root/docs/orgmode/tutorials/quickstart.org"
  "$repo_root/docs/orgmode/howto/editor-integration.org"
  "$repo_root/editors/obsidian/README.md"
)
for file in "${obsidian_sources[@]}"; do
  if ! grep -Fq "Development preview; not listed in Community Plugins" "$file"; then
    printf 'Obsidian preview status missing from %s\n' "$file" >&2
    failed=1
  fi
done

word_preview_sources=(
  "$repo_root/readme_src.org"
  "$repo_root/README.md"
  "$repo_root/docs/orgmode/tutorials/quickstart.org"
  "$repo_root/docs/orgmode/howto/editor-integration.org"
  "$repo_root/editors/word/README.md"
)
for file in "${word_preview_sources[@]}"; do
  if ! grep -Fq "Development preview; not published in AppSource" "$file"; then
    printf 'Word preview status missing from %s\n' "$file" >&2
    failed=1
  fi
done

for pattern in "${unsupported_patterns[@]}"; do
  if grep -Fiq "$pattern" "${public_sources[@]}"; then
    printf 'public docs claim an unavailable distribution: %s\n' "$pattern" >&2
    failed=1
  fi
done

hook_files="$(sed -n 's/^  files: //p' "$repo_root/.pre-commit-hooks.yaml")"
for path in notes.org paper.tex paper.latex README.md notes.markdown index.rst; do
  if ! printf '%s\n' "$path" | grep -Eq -- "$hook_files"; then
    printf 'pre-commit hook skips prose file %s\n' "$path" >&2
    failed=1
  fi
done
for path in requirements.txt docs/requirements-dev.txt CMakeLists.txt LICENSE.txt; do
  if printf '%s\n' "$path" | grep -Eq -- "$hook_files"; then
    printf 'pre-commit hook reflows non-prose file %s\n' "$path" >&2
    failed=1
  fi
done

package_name="$(sed -n '/^\[package\]$/,/^\[/s/^name = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
action_installers="$(grep -o '[A-Za-z0-9_.-]*-installer\.sh' "$repo_root/action.yml" | sort -u)"
if [ "$action_installers" != "$package_name-installer.sh" ]; then
  printf 'action.yml must download %s-installer.sh, found: %s\n' \
    "$package_name" "$action_installers" >&2
  failed=1
fi

action_tmp="$(mktemp -d)"
action_default_files="$(sed -n "/^  files:/,/^  [a-z]/s/^    default: '\(.*\)'/\1/p" "$repo_root/action.yml")"
awk '/- name: Check formatting/ { on = 1; next }
     on && /- name:/ { exit }
     on && /^ {8}/ { sub(/^ {8}/, ""); print }' "$repo_root/action.yml" |
  sed -e "s|\${{ inputs.files }}|$action_default_files|g" \
      -e 's|\${{ inputs.format }}||g' -e 's|\${{ inputs.sarif }}|false|g' \
  > "$action_tmp/check.sh"
mkdir -p "$action_tmp/bin" "$action_tmp/work/docs"
printf '#!/bin/sh\nprintf "%%s\\n" "$@" > "$SNAPPER_ARGS"\n' > "$action_tmp/bin/snapper"
chmod +x "$action_tmp/bin/snapper"
: > "$action_tmp/work/top.md"
: > "$action_tmp/work/docs/inner.md"
if ! (cd "$action_tmp/work" && SNAPPER_ARGS="$action_tmp/args" PATH="$action_tmp/bin:$PATH" \
  bash --noprofile --norc -eo pipefail "$action_tmp/check.sh"); then
  printf 'action.yml check step fails on a tree with no .org or .tex files\n' >&2
  failed=1
fi
for want in top.md docs/inner.md; do
  if ! grep -Fxq "$want" "$action_tmp/args" 2>/dev/null; then
    printf 'action.yml default files skip %s\n' "$want" >&2
    failed=1
  fi
done
if grep -Fq '*' "$action_tmp/args" 2>/dev/null; then
  printf 'action.yml passes an unmatched glob to snapper\n' >&2
  failed=1
fi
rm -rf "$action_tmp"

release_version="$(sed -n '/^\[package\]$/,/^\[/s/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
word_versions=(
  "$repo_root/editors/word/package.json"
  "$repo_root/editors/word/manifest.xml"
  "$repo_root/editors/word/README.md"
  "$repo_root/editors/word/src/taskpane/taskpane.html"
)

wasm_package_versions=(
  "$repo_root/packages/snapper-wasm/package.json"
  "$repo_root/packages/snapper-wasm/package-lock.json"
)

for file in "${wasm_package_versions[@]}"; do
  if ! grep -Fq "\"version\": \"$release_version\"" "$file"; then
    printf 'WASM package version does not match snapper %s: %s\n' \
      "$release_version" "$file" >&2
    failed=1
  fi
done

for file in "${word_versions[@]}"; do
  if ! grep -Fq "$release_version" "$file"; then
    printf 'Word integration version does not match snapper %s: %s\n' \
      "$release_version" "$file" >&2
    failed=1
  fi
done

package_description="$(sed -n '/^\[package\]$/,/^\[/s/^description = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
pypi_description="$(sed -n '/^\[project\]$/,/^\[/s/^description = "\([^"]*\)"/\1/p' "$repo_root/pyproject.toml" | head -n 1)"
conda_summary="$(sed -n 's/^  summary: //p' "$repo_root/conda/recipe.yaml" | head -n 1)"
for summary in "pyproject.toml=$pypi_description" "conda/recipe.yaml=$conda_summary"; do
  if [ "${summary#*=}" != "$package_description" ]; then
    printf 'package summary in %s differs from Cargo.toml: %s\n' \
      "${summary%%=*}" "${summary#*=}" >&2
    failed=1
  fi
done

action_format_help="$(awk '/^  format:/ { on = 1 } on && /description:/ { print; exit }' "$repo_root/action.yml")"
for format in org latex markdown rst plaintext; do
  case "$action_format_help" in
    *"$format"*) ;;
    *)
      printf 'action format input does not list %s\n' "$format" >&2
      failed=1
      ;;
  esac
done

exit "$failed"
