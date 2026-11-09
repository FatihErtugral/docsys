#!/usr/bin/env bash
# docsys-template: 0.18.0
# pre-commit-docs.sh — PreToolUse gate on `git commit`; the decision is made
# by `docsys hook pre-tool-use` (D-051): lint errors block; code without
# documentation is a question asked once per change set, and under
# `commit_policy: require` a refusal every time (R-209). DOCSYS_SKIP=1 bypasses once.
# In a knowledge base the same relay guards raw/: an existing record is never
# overwritten or edited through Write/Edit (R-023, D-076).
docsys_missing() { echo "docsys: not installed, so this commit's documentation is not checked; install the version this tree pins: $docsys_install" >&2; }
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_pin=$(head -n 1 "${DOCS_ROOT:-docs}/.docsys-version" 2>/dev/null || :)
docsys_install="bash ${DOCS_ROOT:-docs}/.docsys-install"
[ -f "${DOCS_ROOT:-docs}/.docsys-install" ] || docsys_install="cargo install docsys --version $docsys_pin --locked"
docsys_pinned="${DOCSYS_HOME:-${HOME:-}/.docsys}/versions/$docsys_pin/bin"
command -v docsys >/dev/null || [ ! -x "$docsys_pinned/docsys" ] || PATH="$docsys_pinned:$PATH"
if ! command -v docsys >/dev/null; then
  docsys_missing
  exit 0
fi
if [ -n "$docsys_pin" ] && [ ! -x "$docsys_pinned/docsys" ]; then
  case "$(docsys --version 2>/dev/null || :)" in
    "docsys $docsys_pin "*) ;;
    *) echo "docsys: this tree pins docsys $docsys_pin; install: $docsys_install" >&2; exit 1 ;;
  esac
fi
exec docsys hook pre-tool-use --root "${DOCS_ROOT:-docs}"
