#!/usr/bin/env bash
# docsys-template: 0.16.0
# session-intent.sh — UserPromptSubmit hook; the routing text once per
# session, from `docsys hook user-prompt-submit` (work types for a project,
# the four organs for a knowledge base — the root's profile decides).
command -v docsys >/dev/null || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_spec=$(sed -n 's/^spec:[[:space:]]*docsys\/0\.\([0-9][0-9]*\).*/\1/p' "${DOCS_ROOT:-docs}/.docmeta.yml" 2>/dev/null | head -n 1)
docsys_impl=$(docsys --version 2>/dev/null | sed -n 's/.*docsys\/0\.\([0-9][0-9]*\).*/\1/p')
if [ -n "$docsys_spec" ] && [ "$docsys_spec" -gt "${docsys_impl:-4}" ]; then
  echo "docsys: this tree needs docsys >= 0.16.0 (it declares docsys/0.$docsys_spec); install: cargo install docsys --version 0.16.0 --locked" >&2
  exit 1
fi
exec docsys hook user-prompt-submit --root "${DOCS_ROOT:-docs}"
