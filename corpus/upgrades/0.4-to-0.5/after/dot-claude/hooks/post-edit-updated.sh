#!/usr/bin/env bash
# docsys-template: 0.16.0
# post-edit-updated.sh — bump `updated:` on the edited docs page (R-052),
# via `docsys hook post-tool-use` (reads the PostToolUse payload on stdin).
command -v docsys >/dev/null || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_spec=$(sed -n 's/^spec:[[:space:]]*docsys\/0\.\([0-9][0-9]*\).*/\1/p' "${DOCS_ROOT:-docs}/.docmeta.yml" 2>/dev/null | head -n 1)
docsys_impl=$(docsys --version 2>/dev/null | sed -n 's/.*docsys\/0\.\([0-9][0-9]*\).*/\1/p')
if [ -n "$docsys_spec" ] && [ "$docsys_spec" -gt "${docsys_impl:-4}" ]; then
  echo "docsys: this tree needs docsys >= 0.16.0 (it declares docsys/0.$docsys_spec); install: cargo install docsys --version 0.16.0 --locked" >&2
  exit 1
fi
exec docsys hook post-tool-use --root "${DOCS_ROOT:-docs}"
