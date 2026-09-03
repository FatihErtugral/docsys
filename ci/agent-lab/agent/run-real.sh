#!/usr/bin/env bash
# agent/run-real.sh <repos.tsv> [--models sonnet,opus] [--round 1|2] [--out <dir>] [--cap <usd>]
#
# The real-repository leg. repos.tsv (gitignored, never committed) has one
# line per repository: codename<TAB>github-owner/name<TAB>size<TAB>round<TAB>notes.
# Every repository is cloned from GitHub into out/real/<codename>/src with
# its remote removed — a push is impossible — and each session works on a
# fresh copy of that clone. Nothing under out/ is ever copied into the
# repository; REPORT-agent.md names codenames only.
#
# Per repository: `docsys adopt` when docs/.docmeta.yml is absent (committed as
# `seed`), the `seed plan` inventory into inventory.txt, then:
#   size S:  F4-seed-real on the largest uncovered feature
#   size M:  F4-seed-real on the two largest uncovered features · F4-learn-real (a fresh base)
#   size L:  round 2 — see ci/agent-lab/README.md
# A long-running script must not read its own file while a person edits it
# (bash reads scripts incrementally): run from a private copy.
if [ -z "${LAB_SELF_COPY:-}" ]; then
  _copy=$(mktemp "${TMPDIR:-/tmp}/docsys-lab-script-XXXXXX")
  cp "${BASH_SOURCE[0]}" "$_copy"
  LAB_SELF_COPY="${BASH_SOURCE[0]}" exec bash "$_copy" "$@"
fi
source "$(dirname "$LAB_SELF_COPY")/../lib.sh"
lab_binary
command -v gh >/dev/null || fail "gh not on PATH (the clones come from GitHub)"
TSV=${1:?repos.tsv}; shift
MODELS="sonnet,opus"; ROUND=1; OUT=""; CAP=60
while [ $# -gt 0 ]; do
  case "$1" in
    --models) MODELS=$2; shift 2 ;;
    --round) ROUND=$2; shift 2 ;;
    --out) OUT=$2; shift 2 ;;
    --cap) CAP=$2; shift 2 ;;
    *) fail "unknown option $1" ;;
  esac
done
[ -n "$OUT" ] || OUT="$OUT_DIR/real/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUT" "$OUT_DIR/real"
COST="$OUT/cost.tsv"; : > "$COST"
RUN="$LAB_DIR/agent/run-task.sh"
over_cap() { awk -v c="$CAP" -F'\t' '{ s += $3 } END { exit !(s > c) }' "$COST"; }
step() { local kind=$1 task=$2 model=$3 code=$4; shift 4
  over_cap && { echo "cap reached" >&2; return 1; }
  local dir="$OUT/$code/$model/$task${TAG:+-$TAG}"
  mkdir -p "$(dirname "$dir")"
  # real histories cost more than fixtures: a wider envelope per model
  local budget=3; case "$model" in *opus*) budget=6 ;; esac
  "$RUN" "$kind" "$task" "$model" --out "$dir" --budget "$budget" "$@" > "$dir.log" 2>&1 || true
  local cost; cost=$(awk -F'\t' '$1 == "cost_usd" { print $2 }' "$dir/meta.tsv" 2>/dev/null); [ -z "$cost" ] || [ "$cost" = "?" ] && cost=0
  printf '%s\t%s\t%s\t%s\n' "$code" "$model" "$task" "$cost" >> "$COST"; tail -1 "$dir.log"; }

IFS=',' read -r -a models <<< "$MODELS"
while IFS=$'\t' read -r code repo size round notes; do
  case "$code" in ''|'#'*|codename) continue ;; esac
  [ "$round" = "$ROUND" ] || continue
  say "$code ($size, $repo)"
  mkdir -p "$OUT/$code"
  src="${LAB_REAL:-$(lab_workdir real-src)}/$code/src"
  if [ ! -d "$src" ]; then
    gh repo clone "$repo" "$src" -- -q || fail "clone $repo"
    ( cd "$src" && git remote remove origin && lab_git_identity )
  fi
  # the working copy carries the repository's own name: the git connector
  # names its namespace after the directory (finding 28)
  prep="$(lab_workdir "real-$code")/${repo##*/}"
  cp -R "$src" "$prep"
  ( cd "$prep" && { [ -f docs/.docmeta.yml ] || docsys adopt > "$OUT/$code/adopt.out" 2>&1 || true; } && git add -A && git commit -qm "lab: adopt docsys" >/dev/null 2>&1 || true; git tag -f seed >/dev/null )
  ( cd "$prep" && docsys seed plan --repo . --root docs > "$OUT/$code/inventory.txt" 2>&1 || true )
  mapfile -t features < <(awk '/^# feature / && /uncovered/ { print $3 }' "$OUT/$code/inventory.txt" | head -2)
  [ ${#features[@]} -gt 0 ] || { echo "no uncovered feature in $code — skipped" | tee -a "$OUT/$code/notes.txt"; continue; }
  for m in "${models[@]}"; do
    case "$size" in
      S) [ "$m" = "${models[0]}" ] || [ "$code" = S1 ] || continue
         TAG="${features[0]}" step real F4-seed-real "$m" "$code" --from "$prep" --var "FEATURE=${features[0]}" ;;
      M) for f in "${features[@]}"; do TAG="$f" step real F4-seed-real "$m" "$code" --from "$prep" --var "FEATURE=$f"; done
         TAG="" step kb F4-learn-real "$m" "$code" --var "REPO_PATH=$prep" ;;
      *) echo "size $size: round 2 (see README)" >> "$OUT/$code/notes.txt" ;;
    esac
  done
done < "$TSV"
printf '\nreal leg: %s\nspent: %s USD\n' "$OUT" "$(awk -F'\t' '{ s += $4 } END { printf("%.2f", s) }' "$COST")"
