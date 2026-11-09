#!/usr/bin/env bash
# docsys-template: 0.17.0
# stop-docs-reminder.sh — end-of-turn reminder; it warns, and under
# `commit_policy: require` it holds the turn once (R-209).
# Reads the working tree and the commits not yet pushed (`docsys hook stop`).
command -v docsys >/dev/null || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_pin=$(head -n 1 "${DOCS_ROOT:-docs}/.docsys-version" 2>/dev/null)
if [ -n "$docsys_pin" ] && ! docsys --version >/dev/null 2>&1; then
  echo "docsys: this tree pins docsys $docsys_pin; install: cargo install docsys --version $docsys_pin --locked" >&2
  exit 1
fi
exec docsys hook stop --root "${DOCS_ROOT:-docs}" --stdin
