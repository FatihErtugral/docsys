# --- docsys documentation gate ---------------------------------------------
@MODE@
# One-off skip: DOCSYS_SKIP=1 git commit ... (under commit_policy: require it leaves a debt item)
docsys_gate_exit=@EXIT@
if [ -z "${DOCSYS_SKIP:-}" ] && command -v docsys >/dev/null; then
  docsys_gate_status=0
  docsys lint --root @ROOT@ || docsys_gate_status=1
  docsys refs --repo . --root @ROOT@ || docsys_gate_status=1
  docsys gate --repo . --root @ROOT@ || docsys_gate_status=1
  if [ "$docsys_gate_status" -ne 0 ] && [ "$docsys_gate_exit" -ne 0 ]; then exit 1; fi
elif [ -n "${DOCSYS_SKIP:-}" ] && command -v docsys >/dev/null; then
  docsys gate --repo . --root @ROOT@ --skipped >/dev/null 2>&1 || :
fi
