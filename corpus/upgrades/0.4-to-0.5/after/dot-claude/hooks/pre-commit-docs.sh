#!/usr/bin/env bash
# docsys-template: 0.16.1
# pre-commit-docs.sh — PreToolUse gate on `git commit`; the decision is made
# by `docsys hook pre-tool-use` (D-051): lint errors block; code without
# documentation is a question asked once per change set, and under
# `commit_policy: require` a refusal every time (R-209). DOCSYS_SKIP=1 bypasses once.
# In a knowledge base the same relay guards raw/: an existing record is never
# overwritten or edited through Write/Edit (R-023, D-076).
command -v docsys >/dev/null || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_pin=$(head -n 1 "${DOCS_ROOT:-docs}/.docsys-version" 2>/dev/null)
if [ -n "$docsys_pin" ] && ! docsys --version >/dev/null 2>&1; then
  echo "docsys: this tree pins docsys $docsys_pin; install: cargo install docsys --version $docsys_pin --locked" >&2
  exit 1
fi
exec docsys hook pre-tool-use --root "${DOCS_ROOT:-docs}"
