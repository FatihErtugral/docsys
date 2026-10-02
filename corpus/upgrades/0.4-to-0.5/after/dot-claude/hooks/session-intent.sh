#!/usr/bin/env bash
# docsys-template: 0.16.0
# session-intent.sh — UserPromptSubmit hook; the routing text once per
# session, from `docsys hook user-prompt-submit` (work types for a project,
# the four organs for a knowledge base — the root's profile decides).
command -v docsys >/dev/null || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_pin=$(head -n 1 "${DOCS_ROOT:-docs}/.docsys-version" 2>/dev/null)
if [ -n "$docsys_pin" ] && ! docsys --version >/dev/null 2>&1; then
  echo "docsys: this tree pins docsys $docsys_pin; install: cargo install docsys --version $docsys_pin --locked" >&2
  exit 1
fi
exec docsys hook user-prompt-submit --root "${DOCS_ROOT:-docs}"
