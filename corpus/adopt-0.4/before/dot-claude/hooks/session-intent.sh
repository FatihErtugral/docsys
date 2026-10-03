#!/usr/bin/env bash
# docsys-template: 0.15.1
# session-intent.sh — UserPromptSubmit hook; the routing text once per
# session, from `docsys hook user-prompt-submit` (work types for a project,
# the four organs for a knowledge base — the root's profile decides).
command -v docsys >/dev/null || exit 0
exec docsys hook user-prompt-submit --root "${DOCS_ROOT:-docs}"
