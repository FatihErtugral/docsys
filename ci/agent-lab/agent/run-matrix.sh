#!/usr/bin/env bash
# agent/run-matrix.sh [--models sonnet,opus] [--out <dir>] [--cap <usd>] [--only <chain>…]
#
# The synthetic matrix: for each model, four chains from the same seeds —
#   kb:         F1-ingest
#   project:    F2-graduate → F2-confirm
#   estate:     F3-learn-ingest
#   brownfield: F4-seed → F4-graduate · F4-kb-pull-ingest (a fresh base over the same repo)
# A docsys/0.5 page carries no verification, so no chain audits (D-130); the
# cross-check a person asks for is the stranger task M-crosscheck.
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
}
chain_project() { local m=$1
  step project F2-graduate "$m" || return
  step project F2-confirm "$m" --from "$OUT/$m/F2-graduate/tree" || return
}
chain_estate() { local m=$1
  step estate F3-learn-ingest "$m" || return
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
printf '\nmatrix: %s\nspent: %s USD (cap %s)\n' "$OUT" "$(spent)" "$CAP"
