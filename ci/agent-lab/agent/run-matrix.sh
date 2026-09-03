#!/usr/bin/env bash
# agent/run-matrix.sh [--models sonnet,opus] [--out <dir>] [--cap <usd>] [--only <chain>…]
#
# The synthetic matrix: for each model, four chains from the same seeds —
#   kb:         F1-ingest → F1-audit (same model) · F1-self-audit-trap
#   project:    F2-graduate → F2-confirm
#   estate:     F3-learn-ingest → F3-learn-audit → (provider moves, fetch) → F3-reaudit
#   brownfield: F4-seed → F4-graduate · F4-kb-pull-ingest (a fresh base over the same repo)
# plus one cross run when both models are asked: opus audits sonnet's ingest.
# Chains run in parallel, steps inside a chain in order; the running cost is
# summed after every session and the matrix stops at --cap.
# A long-running script must not read its own file while a person edits it
# (bash reads scripts incrementally): run from a private copy.
if [ -z "${LAB_SELF_COPY:-}" ]; then
  _copy=$(mktemp "${TMPDIR:-/tmp}/docsys-lab-script-XXXXXX")
  cp "${BASH_SOURCE[0]}" "$_copy"
  LAB_SELF_COPY="${BASH_SOURCE[0]}" exec bash "$_copy" "$@"
fi
source "$(dirname "$LAB_SELF_COPY")/../lib.sh"
lab_binary
MODELS="sonnet,opus"; OUT=""; CAP=90; ONLY=()
while [ $# -gt 0 ]; do
  case "$1" in
    --models) MODELS=$2; shift 2 ;;
    --out) OUT=$2; shift 2 ;;
    --cap) CAP=$2; shift 2 ;;
    --only) ONLY+=("$2"); shift 2 ;;
    *) fail "unknown option $1" ;;
  esac
done
[ -n "$OUT" ] || OUT="$OUT_DIR/agent/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUT"
COST="$OUT/cost.tsv"; : > "$COST"
RUN="$LAB_DIR/agent/run-task.sh"

spent() { awk -F'\t' '{ s += $3 } END { printf("%.2f", s) }' "$COST"; }
over_cap() { awk -v c="$CAP" -F'\t' '{ s += $3 } END { exit !(s > c) }' "$COST"; }
step() { # step <kind> <task> <model> [run-task options…]
  local kind=$1 task=$2 model=$3; shift 3
  if over_cap; then printf 'cap %s reached — skipping %s %s\n' "$CAP" "$task" "$model" >&2; return 1; fi
  local dir="$OUT/$model/$task"
  "$RUN" "$kind" "$task" "$model" --out "$dir" "$@" > "$dir.log" 2>&1 || true
  local cost; cost=$(awk -F'\t' '$1 == "cost_usd" { print $2 }' "$dir/meta.tsv" 2>/dev/null); cost=${cost:-0}
  [ "$cost" = "?" ] && cost=0
  printf '%s\t%s\t%s\n' "$model" "$task" "$cost" >> "$COST"
  tail -1 "$dir.log"
  return 0
}
wanted() { [ ${#ONLY[@]} -eq 0 ] || printf '%s\n' "${ONLY[@]}" | grep -qx -- "$1"; }

chain_kb() { local m=$1
  step kb F1-ingest "$m" || return
  step kb F1-audit "$m" --from "$OUT/$m/F1-ingest/tree" || return
  step kb F1-self-audit-trap "$m" || return
}
chain_project() { local m=$1
  step project F2-graduate "$m" || return
  step project F2-confirm "$m" --from "$OUT/$m/F2-graduate/tree" || return
}
chain_estate() { local m=$1
  step estate F3-learn-ingest "$m" || return
  step estate F3-learn-audit "$m" --from "$OUT/$m/F3-learn-ingest/tree" || return
  # the provider moves; the base fetches (mechanical), then the agent is told lint is red
  local estate; estate="$(cat "$OUT/$m/F3-learn-ingest/workdir")/estate"
  local fix="$OUT/$m/F3-learn-audit/tree" moved; moved="$(lab_workdir reaudit)/moved"
  cp -R "$fix" "$moved"
  ( cd "$estate/relay" && awk '{ if (index($0, "Four attempts, exponential backoff") == 1) print "Six attempts, exponential backoff starting at 200 ms, then a dead letter."; else print }' docs/reference/retry-policy.md > r.tmp && mv r.tmp docs/reference/retry-policy.md && dated_commit . 2026-08-01 "relay: six attempts" "Four was not enough for the slow dependency; six, measured, finishes under 13 s." )
  ( cd "$moved" && docsys fetch --root . >/dev/null 2>&1; docsys inbox pull "$estate/relay" --since 2019-01-01 --root . >/dev/null 2>&1; lab_git_identity; git add -A; git commit -qm "relay moved: fetched, pulled" )
  step estate F3-reaudit "$m" --from "$moved" || return
}
chain_brownfield() { local m=$1
  step brownfield F4-seed "$m" || return
  step brownfield F4-graduate "$m" --from "$OUT/$m/F4-seed/tree" || return
  # the repository under its own name: a path ending in `tree` would name the namespace `tree`
  local named; named="$(lab_workdir kbpull)/ledgerkit"
  cp -R "$OUT/$m/F4-seed/tree" "$named"
  step kb F4-kb-pull-ingest "$m" --var "REPO_PATH=$named" || return
}

IFS=',' read -r -a models <<< "$MODELS"
for m in "${models[@]}"; do
  mkdir -p "$OUT/$m"
  say "model: $m"
  pids=()
  wanted kb && { chain_kb "$m" & pids+=($!); }
  wanted project && { chain_project "$m" & pids+=($!); }
  wanted estate && { chain_estate "$m" & pids+=($!); }
  wanted brownfield && { chain_brownfield "$m" & pids+=($!); }
  for p in "${pids[@]:-}"; do [ -n "$p" ] && wait "$p" || true; done
  printf 'spent so far: %s USD\n' "$(spent)"
done
# the cross run: the second model audits the first model's ingest
if [ ${#models[@]} -ge 2 ] && wanted kb && [ -d "$OUT/${models[0]}/F1-ingest/tree" ]; then
  say "cross: ${models[1]} audits ${models[0]}'s ingest"
  step kb F1-audit "${models[1]}" --from "$OUT/${models[0]}/F1-ingest/tree" --tag "x-${models[0]}" || true
fi
printf '\nmatrix: %s\nspent: %s USD (cap %s)\n' "$OUT" "$(spent)" "$CAP"
