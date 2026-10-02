#!/usr/bin/env bash
# docsys-template: 0.15.1
# post-edit-updated.sh — bump `updated:` on the edited docs page (R-052),
# via `docsys hook post-tool-use` (reads the PostToolUse payload on stdin).
command -v docsys >/dev/null || exit 0
exec docsys hook post-tool-use --root "${DOCS_ROOT:-docs}"
