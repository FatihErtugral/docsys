#!/usr/bin/env bash
# docsys-template: 0.15.1
# pre-commit-docs.sh — PreToolUse gate on `git commit`; the decision is made
# by `docsys hook pre-tool-use` (D-051): lint errors block, the code-without-docs
# question is asked once per change set. DOCSYS_SKIP=1 bypasses once.
# In a knowledge base the same relay guards raw/: an existing record is never
# overwritten or edited through Write/Edit (R-023, D-076).
command -v docsys >/dev/null || exit 0
exec docsys hook pre-tool-use --root "${DOCS_ROOT:-docs}"
