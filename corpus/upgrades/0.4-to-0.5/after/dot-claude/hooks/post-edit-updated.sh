#!/usr/bin/env bash
# docsys-template: 0.16.0
# post-edit-updated.sh — the bookkeeping of the edited docs page: a verified
# page whose body changed turns unverified (R-024), via `docsys hook
# post-tool-use` (reads the PostToolUse payload on stdin).
command -v docsys >/dev/null || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_pin=$(head -n 1 "${DOCS_ROOT:-docs}/.docsys-version" 2>/dev/null)
if [ -n "$docsys_pin" ] && ! docsys --version >/dev/null 2>&1; then
  echo "docsys: this tree pins docsys $docsys_pin; install: cargo install docsys --version $docsys_pin --locked" >&2
  exit 1
fi
exec docsys hook post-tool-use --root "${DOCS_ROOT:-docs}"
