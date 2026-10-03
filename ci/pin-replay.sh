#!/usr/bin/env bash
# ci/pin-replay.sh <clone> <branch> <n> <old-binary> <new-binary>
#
# How much a real change costs the documentation that pins it (D-119): the n
# most recent first-parent commits of <branch> that touched a pinned file are
# replayed once with each binary, and each run prints the pages the commit
# made stale and the docs files and lines a refresh then changed. The old
# binary sees the tree as docsys/0.4 (the page's own `hash:` lines), the new
# one as docsys/0.5 (acknowledgements beside the page).
#
# Manual, not CI: it needs a real clone. It never writes the clone's working
# tree — every commit is replayed in a throwaway worktree of the clone, with
# the HEAD docs tree acknowledged at the commit's parent first, so only the
# commit's own code change can stale a page.
set -euo pipefail

if [ $# -ne 5 ]; then
  echo "usage: $0 <clone> <branch> <n> <old-binary> <new-binary>" >&2
  exit 2
fi
CLONE=$(cd "$1" && pwd)
BRANCH=$2
N=$3
OLD=$(cd "$(dirname "$4")" && pwd)/$(basename "$4")
NEW=$(cd "$(dirname "$5")" && pwd)/$(basename "$5")
ROOT=${DOCS_ROOT:-docs}
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"; git -C "$CLONE" worktree prune' EXIT

g() { git -c commit.gpgsign=false -c core.quotePath=false "$@"; }

pinned="$WORK/pinned"
g -C "$CLONE" grep -h -E '^  - path: ' HEAD -- "$ROOT" | sed 's/^  - path: //' | sort -u >"$pinned" || true
if [ ! -s "$pinned" ]; then
  echo "no pinned path under $ROOT at HEAD" >&2
  exit 2
fi

commits=()
for c in $(g -C "$CLONE" rev-list --first-parent "$BRANCH"); do
  p=$(g -C "$CLONE" rev-parse -q --verify "$c^1" 2>/dev/null) || continue
  # through a file: `grep -q` stops reading early, and under pipefail the
  # writer's SIGPIPE would read as "no pinned file changed"
  g -C "$CLONE" diff --name-only "$p" "$c" >"$WORK/changed"
  if grep -Fxq -f "$pinned" "$WORK/changed"; then
    commits+=("$c")
  fi
  [ "${#commits[@]}" -ge "$N" ] && break
done

# pages with an R-111 finding, one per line
stale_pages() {
  (cd "$1" && "$2" lint --root "$ROOT" --repo . || true) | awk '/^ERROR R-111 /{print $3}' | sort -u
}

replay() { # <binary> <spec minor> <commit> <label>
  local bin=$1 spec=$2 c=$3 label=$4 p wt skipped=0 before after newly files lines
  p=$(g -C "$CLONE" rev-parse "$c^1")
  wt="$WORK/wt"
  rm -rf "$wt"
  g -C "$CLONE" worktree add -q --detach "$wt" "$p"
  rm -rf "${wt:?}/$ROOT"
  g -C "$CLONE" archive HEAD "$ROOT" | tar -x -C "$wt"
  awk -v s="spec: docsys/0.$spec" '/^spec:/ { print s; next } { print }' \
    "$wt/$ROOT/.docmeta.yml" >"$WORK/dm" && mv "$WORK/dm" "$wt/$ROOT/.docmeta.yml"
  # acknowledge every pin at the parent; a page with a region the parent
  # cannot read is skipped and counted
  while IFS= read -r f; do
    page=${f#"$wt/$ROOT/"}
    if ! (cd "$wt" && "$bin" pin --refresh "${page%.md}" --repo . --root "$ROOT" >/dev/null 2>&1); then
      skipped=$((skipped + 1))
    fi
  done < <(grep -rl --include='*.md' '^verifies:' "$wt/$ROOT" || true)
  g -C "$wt" add -A
  g -C "$wt" -c user.email=replay@example.invalid -c user.name=replay commit -qm "acknowledged at the parent"
  before=$(stale_pages "$wt" "$bin")
  # the commit's own change, the docs tree excluded
  g -C "$CLONE" diff --binary "$p" "$c" -- . ":(exclude)$ROOT" | g -C "$wt" apply --index
  after=$(stale_pages "$wt" "$bin")
  newly=$(comm -13 <(printf '%s\n' "$before" | sed '/^$/d') <(printf '%s\n' "$after" | sed '/^$/d'))
  for page in $newly; do
    (cd "$wt" && "$bin" pin --refresh "${page%.md}" --repo . --root "$ROOT" >/dev/null 2>&1) || true
  done
  g -C "$wt" add -A -- "$ROOT"
  files=$(g -C "$wt" diff --cached --name-only -- "$ROOT" | grep -c . || true)
  lines=$(g -C "$wt" diff --cached --numstat -- "$ROOT" | awk '{ n += $1 + $2 } END { print n + 0 }')
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "${c:0:9}" "$label" \
    "$(printf '%s\n' "$newly" | grep -c . || true)" "$files" "$lines" "$skipped"
  g -C "$CLONE" worktree remove --force "$wt"
}

printf 'commit\tbinary\tpages_staled\tdocs_files_changed\tdocs_lines_changed\tpages_skipped\n'
out="$WORK/rows"
: >"$out"
for c in "${commits[@]}"; do
  replay "$OLD" 4 "$c" old | tee -a "$out"
  replay "$NEW" 5 "$c" new | tee -a "$out"
done
awk -F'\t' '{ s[$2] += $3; f[$2] += $4; l[$2] += $5 }
  END { for (b in s) printf "total\t%s\t%d\t%d\t%d\t-\n", b, s[b], f[b], l[b] }' "$out" | sort
