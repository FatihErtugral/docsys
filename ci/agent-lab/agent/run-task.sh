#!/usr/bin/env bash
# agent/run-task.sh <kind> <task> <model> [--from <fixture>] [--out <dir>]
#                   [--var KEY=VALUE]… [--budget <usd>] [--tag <label>]
#
# One headless session: a fresh fixture of <kind> (kb | project | estate |
# brownfield) or a copy of a previous result (--from), the task text as the
# person's message, the installed layer as the only instruction (D-087);
# then the mechanical capture (commits, diff, lint, status, hook traces, cost,
# turns, model id, binary sha) and agent/checks.sh → auto.tsv.
# A long-running script must not read its own file while a person edits it
# (bash reads scripts incrementally): run from a private copy.
if [ -z "${LAB_SELF_COPY:-}" ]; then
  _copy=$(mktemp "${TMPDIR:-/tmp}/docsys-lab-script-XXXXXX")
  cp "${BASH_SOURCE[0]}" "$_copy"
  LAB_SELF_COPY="${BASH_SOURCE[0]}" exec bash "$_copy" "$@"
fi
source "$(dirname "$LAB_SELF_COPY")/../lib.sh"
lab_binary
command -v claude >/dev/null || fail "claude not on PATH"
KIND=${1:?kind}; TASK=${2:?task}; MODEL=${3:?model}; shift 3
FROM=""; OUT=""; BUDGET=""; TAG=""; VARS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --from) FROM=$2; shift 2 ;;
    --out) OUT=$2; shift 2 ;;
    --var) VARS+=("$2"); shift 2 ;;
    --budget) BUDGET=$2; shift 2 ;;
    --tag) TAG=$2; shift 2 ;;
    *) fail "unknown option $1" ;;
  esac
done
[ -n "$OUT" ] || OUT="$OUT_DIR/agent/$(date +%Y%m%d-%H%M%S)/$TASK-$MODEL${TAG:+-$TAG}"
mkdir -p "$OUT"
WORK=$(lab_workdir "task-$TASK-$MODEL")
FIX="$WORK/fixture"
[ -f "$LAB_DIR/tasks/$TASK.md" ] || fail "no task $TASK"
case "$MODEL" in
  *opus*) BUDGET=${BUDGET:-5} ;;
  *) BUDGET=${BUDGET:-1.5} ;;
esac

# ── the fixture ──────────────────────────────────────────────────────────
if [ -n "$FROM" ]; then
  cp -R "$FROM" "$FIX"
else
  case "$KIND" in
    kb) "$LAB_DIR/fixtures/gen-kb.sh" "$FIX" >/dev/null ;;
    project) "$LAB_DIR/fixtures/gen-project.sh" "$FIX" >/dev/null ;;
    estate)
      "$LAB_DIR/fixtures/gen-estate.sh" "$WORK/estate" >/dev/null
      docsys assistant --root "$FIX" --projects "$WORK/estate" --domains coding --since 2019-01-01 > "$OUT/assistant.out" 2>&1 \
        || fail "assistant: $(cat "$OUT/assistant.out")"
      ( cd "$FIX" && lab_git_identity && git add -A && git commit -qm "assistant: the base, three projects, their records" )
      ;;
    brownfield)
      "$LAB_DIR/fixtures/gen-brownfield.sh" "$FIX" >/dev/null
      ( cd "$FIX" && docsys adopt > "$OUT/adopt.out" 2>&1 && git add -A && git commit -qm "adopt docsys" )
      # one feature already covered (`cli`), so the seed refusal path is real
      ( cd "$FIX" && docsys page new reference cli --title "The ledgerkit CLI" --root docs >/dev/null \
        && awk '{ if (index($0, "<!-- opening:") == 1) print "This page is the command surface of the ledgerkit CLI; read it before scripting against it."; else print }' docs/reference/cli.md > "$WORK/cli.tmp" && mv "$WORK/cli.tmp" docs/reference/cli.md \
        && printf -- '\n- [[reference/cli|The ledgerkit CLI]] -- the command surface.\n' >> docs/index.md \
        && git add -A && git commit -qm "docs: cli reference" )
      ;;
    *) fail "unknown kind $KIND (kb | project | estate | brownfield)" ;;
  esac
fi
case "$KIND" in
  kb|estate) ROOT="." ;;
  *) ROOT="docs" ;;
esac
( cd "$FIX" && lab_git_identity && git tag -f seed >/dev/null )
assert_outside_repo "$WORK"
printf '%s\n' "$WORK" > "$OUT/workdir"

# ── the message ──────────────────────────────────────────────────────────
prompt="$(cat "$LAB_DIR/tasks/_preamble.md" "$LAB_DIR/tasks/$TASK.md")"
for kv in "${VARS[@]:-}"; do
  [ -n "$kv" ] || continue
  key=${kv%%=*}; val=${kv#*=}
  prompt=$(printf '%s' "$prompt" | awk -v k="\${$key}" -v v="$val" '{ i = index($0, k); while (i > 0) { $0 = substr($0, 1, i - 1) v substr($0, i + length(k)); i = index($0, k) } print }')
done
printf '%s\n' "$prompt" > "$OUT/prompt.md"
{
  printf 'kind\t%s\ntask\t%s\nmodel_requested\t%s\nbudget_usd\t%s\nroot\t%s\nfrom\t%s\n' "$KIND" "$TASK" "$MODEL" "$BUDGET" "$ROOT" "${FROM:-}"
  printf 'binary_sha256\t%s\nstarted\t%s\n' "$(sha256_of "$(command -v docsys)")" "$(date -u +%FT%TZ)"
} > "$OUT/meta.tsv"

# ── the session ──────────────────────────────────────────────────────────
export TMPDIR="$WORK/tmp"; mkdir -p "$TMPDIR"
start=$(date +%s)
rc=0
( cd "$FIX" && claude -p "$prompt" --model "$MODEL" --permission-mode bypassPermissions \
    --output-format stream-json --verbose --no-session-persistence \
    --max-turns 80 --max-budget-usd "$BUDGET" \
    > "$OUT/transcript.jsonl" 2> "$OUT/stderr.log" ) || rc=$?
end=$(date +%s)
printf 'exit\t%s\nwall_seconds\t%s\nfinished\t%s\n' "$rc" "$((end - start))" "$(date -u +%FT%TZ)" >> "$OUT/meta.tsv"

# ── the capture (all mechanical) ─────────────────────────────────────────
cd "$FIX"
git log --format='%h %s' seed..HEAD > "$OUT/commits.txt" || true
git status --porcelain > "$OUT/leftovers.txt" || true
git diff seed --stat > "$OUT/diffstat.txt" || true
git diff seed > "$OUT/diff.patch" || true
git log --format='%B' seed..HEAD > "$OUT/commit-messages.txt" || true
(docsys lint --root "$ROOT" || true) > "$OUT/lint.txt"
(docsys lint --root "$ROOT" --json || true) > "$OUT/lint.json"
if [ "$ROOT" = "." ]; then (docsys status --root . || true) > "$OUT/status.txt"; fi
# pipefail-safe extraction: grep may match nothing (exit 1) and must never
# feed `head` (SIGPIPE); the last match is read with tail
last() { (grep -o -- "$1" "$2" 2>/dev/null || true) | tail -1; }
count() { (grep -c -- "$1" "$2" 2>/dev/null || true) | tail -1; }
model_id=$(last '"model":"claude-[^"]*"' "$OUT/transcript.jsonl" | cut -d'"' -f4)
cost=$(last '"total_cost_usd":[0-9.]*' "$OUT/transcript.jsonl" | cut -d: -f2)
turns=$(last '"num_turns":[0-9]*' "$OUT/transcript.jsonl" | cut -d: -f2)
dur=$(last '"duration_ms":[0-9]*' "$OUT/transcript.jsonl" | cut -d: -f2)
r023=$(( $(count 'R-023' "$OUT/transcript.jsonl") + $(count 'R-023' "$OUT/stderr.log") ))
routing=$(count 'session-doc-routing' "$OUT/transcript.jsonl")
gate=$(count 'docsys documentation gate\|GATE ' "$OUT/transcript.jsonl")
{
  printf 'model_id\t%s\ncost_usd\t%s\nturns\t%s\nduration_ms\t%s\n' "${model_id:-?}" "${cost:-?}" "${turns:-?}" "${dur:-?}"
  printf 'commits\t%s\nleftovers\t%s\nhook_r023\t%s\nhook_routing\t%s\nhook_gate\t%s\n' \
    "$(wc -l < "$OUT/commits.txt" | tr -d ' ')" "$(wc -l < "$OUT/leftovers.txt" | tr -d ' ')" "$r023" "${routing:-0}" "${gate:-0}"
  printf 'lint\t%s\n' "$( (grep -- '^-- ' "$OUT/lint.txt" || true) | tail -1)"
} >> "$OUT/meta.tsv"
# the final result event, and the report text inside it (best effort: JSON escapes stay)
grep '"type":"result"' "$OUT/transcript.jsonl" | tail -1 > "$OUT/result.json" || true
awk 'match($0, /"result":"/) { s = substr($0, RSTART + 10); sub(/","[a-z_]+":.*$/, "", s); gsub(/\\n/, "\n", s); print s }' "$OUT/result.json" | head -c 6000 > "$OUT/report.txt" || true

"$LAB_DIR/agent/checks.sh" "$KIND" "$TASK" "$OUT" || true
# the tree, as the session left it (its .git included), beside the artifacts
cp -R "$FIX" "$OUT/tree"
printf 'run-task: %s %s → %s (exit %s, cost %s, turns %s)\n' "$TASK" "$MODEL" "$OUT" "$rc" "${cost:-?}" "${turns:-?}"
