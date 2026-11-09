#!/usr/bin/env bash
# docsys-template: 0.18.0
# stop-docs-reminder.sh — end-of-turn reminder; it warns, and under
# `commit_policy: require` it holds the turn once (R-209).
# Reads the working tree and the commits not yet pushed (`docsys hook stop`).
docsys_missing() { :; }
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
exec docsys hook stop --root "${DOCS_ROOT:-docs}" --stdin
