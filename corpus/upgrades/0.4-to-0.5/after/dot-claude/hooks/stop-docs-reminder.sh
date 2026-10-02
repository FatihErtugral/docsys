#!/usr/bin/env bash
# docsys-template: 0.15.1
# stop-docs-reminder.sh — end-of-turn nudge; warns, never blocks (R-150).
# Reads the working tree and the commits not yet pushed (`docsys hook stop`).
command -v docsys >/dev/null || exit 0
# no nudge on the release branch
[ "$(git branch --show-current)" = release ] && exit 0
exec docsys hook stop --root "${DOCS_ROOT:-docs}" --stdin
