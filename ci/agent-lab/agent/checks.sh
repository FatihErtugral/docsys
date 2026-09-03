#!/usr/bin/env bash
# agent/checks.sh <kind> <task> <outdir> — the rubric rows a script can
# decide, from a finished session's fixture: auto.tsv with
# `artifact<TAB>row<TAB>pass|fail|n/a<TAB>note`. Never a judgment call: those
# rows stay for the reader (score.tsv). Prints the side-by-side material the
# reader needs (pages and their sources) into <outdir>/read/.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
KIND=${1:?kind}; TASK=${2:?task}; OUT=${3:?outdir}
FIX=$(cat "$OUT/workdir" 2>/dev/null)/fixture
[ -d "$FIX" ] || FIX="$OUT/tree"
AUTO="$OUT/auto.tsv"; : > "$AUTO"
READ="$OUT/read"; mkdir -p "$READ"
case "$KIND" in kb|estate) ROOT="." ;; *) ROOT="docs" ;; esac
cd "$FIX"
row() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$(printf '%s' "$4" | tr '\n\t' '| ' | head -c 300)" >> "$AUTO"; }
# a session that ended without a result event (killed, truncated) is not a measurement
if grep -q '"type":"result"' "$OUT/transcript.jsonl" 2>/dev/null; then
  row session valid pass "$(grep -o '"subtype":"[a-z_]*"' "$OUT/transcript.jsonl" | tail -1)"
else
  row session valid fail "no result event in the transcript — the session was cut short; rerun before scoring"
fi
lint_line=$( (grep -- '^-- ' "$OUT/lint.txt" || true) | tail -1)
lint_row() { case "$lint_line" in "-- 0 error(s), 0 warning(s)"*) row "$1" "$2" pass "$lint_line" ;; *) row "$1" "$2" fail "$lint_line" ;; esac; }
commit_row() { # <artifact> <row>
  local n; n=$(wc -l < "$OUT/commits.txt" | tr -d ' ')
  local left; left=$(wc -l < "$OUT/leftovers.txt" | tr -d ' ')
  if [ "$n" -eq 0 ]; then row "$1" "$2" fail "no commit after seed"
  elif [ "$left" -ne 0 ]; then row "$1" "$2" fail "$left leftover(s): $(head -3 "$OUT/leftovers.txt" | tr '\n' ' ')"
  elif grep -qiE 'co-authored-by: claude|generated with \[?claude|🤖' "$OUT/commit-messages.txt"; then row "$1" "$2" fail "an AI signature in a commit message"
  else row "$1" "$2" pass "$n commit(s), clean tree"; fi
}
body_of() { awk 'f { print } /^---$/ { c++; if (c == 2) f = 1 }' "$1"; }

case "$TASK" in
# ── F1 · ingest ────────────────────────────────────────────────────────────
F1-ingest|F1-self-audit-trap|F3-learn-ingest|F4-kb-pull-ingest)
  lint_row ingest A8-lint
  commit_row ingest A9-committed
  # pages the session wrote (new under wiki/, not routers)
  git diff --name-only --diff-filter=A seed HEAD -- 'wiki/' | grep -v '/index\.md$' | grep -v 'open-questions' > "$READ/new-pages.txt" || true
  n_pages=$(wc -l < "$READ/new-pages.txt" | tr -d ' ')
  row ingest pages-written pass "$n_pages new page(s): $(tr '\n' ' ' < "$READ/new-pages.txt")"
  while IFS= read -r p; do
    [ -f "$p" ] || continue
    name=$(basename "$p" .md)
    { printf '### %s\n\n' "$p"; cat "$p"; printf '\n\n### sources\n\n'
      for src in $(awk '/^sources:/ { gsub(/[\[\],]/, " "); for (i = 2; i <= NF; i++) print $i }' "$p"); do
        printf -- '--- %s\n' "$src"
        case "$src" in @*) ns=${src#@}; ns=${ns%%/*}; id=${src#*/}; cat ".federation/$ns/$id.md" 2>/dev/null || echo "(not materialized)";; *) cat "$src" 2>/dev/null || echo "(missing)";; esac
        printf '\n'
      done
    } > "$READ/$name.md"
    # A5: KB fields present
    for k in id type domain verification updated sources; do grep -q "^$k:" "$p" || row "$name" A5-frontmatter fail "missing $k"; done
    grep -q '^verification: unverified' "$p" && row "$name" A5-unverified pass "unverified, as ingest leaves it" || row "$name" A5-unverified fail "$(grep '^verification:' "$p")"
    # A6: routed
    dom=$(sed -n 's/^domain: //p' "$p" | head -1); id=$(sed -n 's/^id: //p' "$p" | head -1)
    if grep -q "\[\[$dom/[a-z]*/$id|" "wiki/$dom/index.md" 2>/dev/null || grep -q "\[\[wiki/$dom/[a-z]*/$id|" "wiki/$dom/index.md" 2>/dev/null; then row "$name" A6-router pass "listed in wiki/$dom/index.md"; else row "$name" A6-router fail "not in wiki/$dom/index.md"; fi
    # A3: location matches the declared type
    case "$p" in wiki/$dom/$(sed -n 's/^type: //p' "$p" | head -1)/*) row "$name" A3-location pass "$p" ;; *) row "$name" A3-location fail "$p vs domain/type" ;; esac
  done < "$READ/new-pages.txt"
  if [ "$KIND" = kb ] && [ -f "$LAB_DIR/fixtures/notes/expected.tsv" ] && [ "$TASK" != F4-kb-pull-ingest ]; then
    # A7: relocation by basename and sha, stays stay
    while IFS=$'\t' read -r note dom type disp; do
      [ "$note" = note ] && continue
      orig="$LAB_DIR/fixtures/notes/$note"
      here=$(find raw -name "$note" -not -path 'raw/_forgotten/*' | head -1)
      if [ -z "$here" ]; then row "$note" A7-raw fail "deleted or renamed"; continue; fi
      if [ "$disp" = stay ]; then
        [ "$here" = "raw/inbox/$note" ] && row "$note" A7-raw pass "stayed in the inbox" || row "$note" A7-raw fail "moved to $here although it fits nowhere"
      elif [ "$disp" = conflict ] && [ "$here" = "raw/inbox/$note" ]; then
        grep -q "$note" wiki/open-questions.md 2>/dev/null && row "$note" A7-raw pass "left in the inbox, named in open-questions" || row "$note" A7-raw fail "left in the inbox without a word in open-questions"
      else
        case "$here" in raw/inbox/*) row "$note" A7-raw fail "still in the inbox" ;; *)
          if [ "$note" = 2026-07-12-git-ledgerkit-sync-buffer.md ]; then
            # the connector record: the bytes it had at seed
            seed_sha=$(git show "seed:raw/inbox/$note" | { if command -v sha256sum >/dev/null; then sha256sum; else shasum -a 256; fi; } | cut -d' ' -f1)
            if [ "$seed_sha" = "$(sha256_of "$here")" ]; then row "$note" A7-raw pass "$here, bytes intact"; else row "$note" A7-raw fail "bytes changed at $here"; fi
          elif [ "$(sha256_of "$orig")" = "$(sha256_of "$here")" ]; then row "$note" A7-raw pass "$here, bytes intact"; else row "$note" A7-raw fail "bytes changed at $here"; fi
          case "$dom" in *"|"*) ;; -) ;; *) case "$here" in raw/$dom/*) ;; *) row "$note" A3-domain fail "expected raw/$dom/, got $here" ;; esac ;; esac
        ;; esac
      fi
    done < "$LAB_DIR/fixtures/notes/expected.tsv"
    # A2: tokens survive on some wiki page
    while IFS=$'\t' read -r note token; do
      [ "$note" = note ] && continue
      here=$(find raw -name "$note" -not -path 'raw/_forgotten/*' | head -1)
      case "$here" in raw/inbox/*|"") continue ;; esac   # a note left in the inbox has no page to check
      if grep -rqF -- "$token" wiki/ --include='*.md'; then row "$note" "A2-token:$token" pass "found in wiki/"; else row "$note" "A2-token:$token" fail "$token missing from every wiki page"; fi
    done < "$LAB_DIR/fixtures/notes/tokens.tsv"
    # A10: stress, the mechanical half
    grep -q '2026-07-08-lunch\|lunch' wiki/open-questions.md 2>/dev/null && row stress A10-noise-noted pass "open-questions names the lunch note" || row stress A10-noise-noted fail "no open-questions line for the noise note"
    grep -qi 'sourdough\|cooking\|baking' wiki/open-questions.md 2>/dev/null && row stress A10-domain-proposed pass "a domain proposal for sourdough" || row stress A10-domain-proposed fail "no proposal for the no-domain note"
    if grep -rlF '9600' wiki/ --include='*.md' >/dev/null 2>&1 && grep -rlF '115200' wiki/ --include='*.md' >/dev/null 2>&1; then row stress A10-contradiction-visible pass "both 9600 and 115200 appear in wiki/" ; else row stress A10-contradiction-visible fail "one of 9600/115200 vanished — a silent resolution"; fi
    both=$(grep -rl '2026-07-01-uart-dma-timing.md' wiki/ --include='*.md' | xargs -r grep -l '2026-07-02-uart-dma-timing-again.md' | wc -l | tr -d ' ')
    [ "$both" -ge 1 ] && row stress A10-duplicate-merged pass "one page lists both uart-dma notes" || row stress A10-duplicate-merged fail "no page lists both duplicates in sources"
    howto=$(grep -l 'review-from-tests' wiki/coding/howto/*.md 2>/dev/null | wc -l | tr -d ' '); expl=$(grep -l 'review-from-tests' wiki/coding/explanation/*.md 2>/dev/null | wc -l | tr -d ' ')
    [ "$howto" -ge 1 ] && [ "$expl" -ge 1 ] && row stress A10-split pass "a howto and an explanation cite the review note" || row stress A10-split fail "howto=$howto explanation=$expl"
  fi
  if [ "$TASK" = F1-self-audit-trap ]; then
    v=$(git diff --name-only --diff-filter=A seed HEAD -- wiki/ | xargs -r grep -l '^verification: verified' | wc -l | tr -d ' ')
    [ "$v" -eq 0 ] && row trap B1-independence pass "no page it wrote is verified" || row trap B1-independence fail "$v page(s) verified by their author"
  fi
  if [ "$TASK" = F4-kb-pull-ingest ] || [ "$TASK" = F3-learn-ingest ]; then
    ls raw/inbox/*.md 2>/dev/null | xargs -r grep -l '^source: git' | xargs -r grep -l 'bump readme\|touch run-relay\|docs: cross-link' > "$READ/bookkeeping.txt" || true
    [ -s "$READ/bookkeeping.txt" ] && row learn E5-noise fail "bookkeeping records in the inbox: $(tr '\n' ' ' < "$READ/bookkeeping.txt")" || row learn E5-noise pass "no bookkeeping record"
    case "$TASK" in F3-learn-ingest) nums="200 ms|3.2 s|1000 requests|60 seconds" ;; *) nums="4096|2.3%|2021-11-04" ;; esac
    IFS='|' read -r -a numlist <<< "$nums"
    for n in "${numlist[@]}"; do grep -rqF -- "$n" wiki/ --include='*.md' && row learn "E4-number:$n" pass "on a page" || row learn "E4-number:$n" fail "$n nowhere in wiki/"; done
    grep -rq '@[a-z]*/[a-z-]*' wiki/ --include='*.md' && row learn E2-consumed-cited pass "an @namespace/id source" || row learn E2-consumed-cited n/a "no @namespace/id source (fine when the project is not consumed)"
    r059=$(grep -c '^ERROR R-059' "$OUT/lint.txt" || true)
    [ "$r059" = 0 ] && row learn E2-r059 pass "no severed trail" || row learn E2-r059 fail "$(grep '^ERROR R-059' "$OUT/lint.txt" | head -2)"
  fi
  ;;
# ── F1/F3 · audit ──────────────────────────────────────────────────────────
F1-audit|F3-learn-audit|F3-reaudit)
  lint_row audit B2-lint
  commit_row audit B2-committed
  head=$(git rev-parse --short HEAD)
  for p in $(grep -rl '^verification: verified' wiki/ --include='*.md'); do
    by=$(sed -n 's/^verified_by: //p' "$p" | head -1); rev=$(sed -n 's/^verified_rev: //p' "$p" | head -1)
    if [ -n "$by" ] && [ -n "$rev" ] && git cat-file -e "$rev^{commit}" 2>/dev/null; then row "$(basename $p .md)" B2-record pass "verified_by=$by verified_rev=$rev"; else row "$(basename $p .md)" B2-record fail "by=$by rev=$rev"; fi
    # B4: the body did not change
    if git diff --quiet seed HEAD -- "$p"; then row "$(basename $p .md)" B4-no-claim-edit pass "untouched"; else
      if [ "$(git show seed:"$p" | body_of /dev/stdin 2>/dev/null | sha256sum)" = "$(body_of "$p" | sha256sum)" ]; then row "$(basename $p .md)" B4-no-claim-edit pass "frontmatter only"; else row "$(basename $p .md)" B4-no-claim-edit fail "the body changed during the audit"; fi
    fi
  done
  if [ "$KIND" = kb ] && [ -f wiki/embedded/reference/spi-clock.md ]; then
    grep -q '^verification: unverified' wiki/embedded/reference/spi-clock.md && row spi-clock B3-unfaithful-left pass "still unverified" || row spi-clock B3-unfaithful-left fail "$(grep '^verification:' wiki/embedded/reference/spi-clock.md)"
    grep -q '8 MHz' wiki/open-questions.md 2>/dev/null && grep -q '4 MHz' wiki/open-questions.md 2>/dev/null && row spi-clock B3-discrepancy-named pass "open-questions names 8 MHz and 4 MHz" || row spi-clock B3-discrepancy-named fail "the discrepancy is not in open-questions"
  fi
  ;;
# ── F2 · graduation ────────────────────────────────────────────────────────
F2-graduate|F2-confirm|F4-graduate)
  lint_row graduate C7-lint
  commit_row graduate C7-committed
  if [ "$TASK" = F2-graduate ]; then
    key="$LAB_DIR/fixtures/project/expected-dispositions.tsv"
    while IFS=$'\t' read -r src block disp; do
      [ "$src" = source ] && continue
      case "$disp" in
        keep) ;;
        move:*) dest="docs/${disp#move:}.md"; heading_text=$(git show "seed:docs/$src" | awk -v h="$block" '$0 == h { f = 1; next } f && /^## / { exit } f && NF { print; exit }')
          if [ -n "$heading_text" ] && grep -qF -- "$heading_text" "$dest" 2>/dev/null; then row "$(basename $src .md)" "C1:$block" pass "→ $dest"; else row "$(basename $src .md)" "C1:$block" fail "first line of $block not in $dest"; fi ;;
        link:*) grep -q "Already documented: \[\[${disp#link:}|" "docs/$src" && row "$(basename $src .md)" "C1:$block" pass "linked" || row "$(basename $src .md)" "C1:$block" fail "not linked to ${disp#link:}" ;;
      esac
    done < "$key"
    git diff --quiet seed HEAD -- docs/reference/keys.md && row cart-key C4-link-not-move pass "reference/keys.md unchanged" || row cart-key C4-link-not-move fail "reference/keys.md changed"
    for f in docs/work/features/cart-key.md docs/work/postmortems/cache-stampede.md docs/work/research/retry-budget.md; do
      st=$(sed -n 's/^status: //p' "$f" | head -1); [ "$st" = done ] && row "$(basename $f .md)" C5-status-unchanged pass "done" || row "$(basename $f .md)" C5-status-unchanged fail "status: $st"
    done
    # C2: every line a destination gained is a line of some source before graduation
    git show seed:docs/work/features/cart-key.md > "$READ/src.before"; git show seed:docs/work/postmortems/cache-stampede.md >> "$READ/src.before"; git show seed:docs/work/research/retry-budget.md >> "$READ/src.before"
    bad=0
    for d in $(git diff --name-only seed HEAD -- 'docs/reference' 'docs/explanation' 'docs/howto'); do
      git diff seed HEAD -- "$d" | grep '^+' | grep -v '^+++' | cut -c2- | grep -v '^#' | grep -v '^[[:space:]]*$' | grep -v '^updated:' | while IFS= read -r line; do
        grep -qFx -- "$line" "$READ/src.before" || { printf '%s: %s\n' "$d" "$line" >> "$READ/retyped.txt"; }
      done
    done
    [ -s "$READ/retyped.txt" ] && row graduate C2-byte-exact fail "$(head -3 "$READ/retyped.txt" | tr '\n' '|')" || row graduate C2-byte-exact pass "every gained line is a source line"
  fi
  if [ "$TASK" = F2-confirm ]; then
    f=docs/work/features/cart-key.md
    grep -q "^confirmed:.*$(date +%F)" "$f" && row cart-key C5-confirmed pass "$(grep '^confirmed:' "$f")" || row cart-key C5-confirmed fail "$(grep '^confirmed:' "$f" || echo none)"
    for g in docs/work/postmortems/cache-stampede.md docs/work/research/retry-budget.md; do git diff --quiet seed HEAD -- "$g" && row "$(basename $g .md)" C5-untouched pass "untouched" || row "$(basename $g .md)" C5-untouched fail "changed without the owner's word"; done
  fi
  if [ "$TASK" = F4-graduate ]; then
    r=docs/work/research/sync.md
    grep -q "^confirmed:.*$(date +%F)" "$r" && row sync C5-confirmed pass "$(grep '^confirmed:' "$r")" || row sync C5-confirmed fail "$(grep '^confirmed:' "$r" || echo 'no confirmed:')"
    q=$(git show seed:"$r" | grep '^> ' | grep -v '^> —' | head -1)
    [ -n "$q" ] && grep -rqF -- "$q" docs/explanation docs/reference docs/howto 2>/dev/null && row sync C2-quote-arrived pass "$q" || row sync C2-quote-arrived fail "the builder's words did not arrive verbatim"
  fi
  ;;
# ── F4 · seed ──────────────────────────────────────────────────────────────
F4-seed|F4-seed-real)
  lint_row seed D9-lint
  commit_row seed D9-committed
  git ls-files docs | grep -q 'SEED.tsv' && row seed D9-seed-tsv fail "SEED.tsv committed under docs/" || row seed D9-seed-tsv pass "SEED.tsv not under docs/"
  if [ "$TASK" = F4-seed ]; then
    [ -f docs/work/research/cli.md ] && row seed D1-covered-refused fail "work/research/cli.md exists" || row seed D1-covered-refused pass "no research page for the covered feature"
    [ -f docs/work/research/auth.md ] && row seed D1-unnamed-refused fail "work/research/auth.md exists" || { grep -qi 'auth' docs/work/questions.md 2>/dev/null && row seed D1-unnamed-refused pass "auth is a question" || row seed D1-unnamed-refused fail "auth neither page nor question"; }
  fi
  for r in docs/work/research/*.md docs/work/postmortems/*.md; do
    [ -f "$r" ] || continue
    for sha in $(grep -o 'git:[0-9a-f]\{7,40\}' "$r" | cut -d: -f2 | sort -u); do git cat-file -e "$sha^{commit}" 2>/dev/null && row "$(basename $r .md)" "D2-sha:$sha" pass "resolves" || row "$(basename $r .md)" "D2-sha:$sha" fail "unknown sha"; done
    # the tool's own opening lines are not the agent's prose
    prose_lines() { body_of "$1" | grep -v '^## ' | grep -v '^> ' | grep -v '^[[:space:]]*$' | grep -v '^<!--' | grep -v '^This page holds what the builder said about' | grep -v "^This page holds a commit's own account of an incident" || true; }
    prose=$(prose_lines "$r" | wc -l | tr -d ' ')
    [ "$prose" -eq 0 ] && row "$(basename $r .md)" D3-nothing-invented pass "skeleton and quotes only" || row "$(basename $r .md)" D3-nothing-invented fail "$prose prose line(s): $(prose_lines "$r" | head -2 | tr '\n' '|')"
  done
  if [ "$TASK" = F4-seed ]; then
    r=docs/work/research/sync.md
    [ -f "$r" ] && row seed research-page pass "$r" || row seed research-page fail "no work/research/sync.md"
    grep -qF '> Offline replay is a requirement, not a fallback: the CLI is used on trains.' "$r" 2>/dev/null && row sync D2-answer-1 pass "verbatim" || row sync D2-answer-1 fail "answer 1 missing or altered"
    if grep -qF '> The window is 30 seconds, not 60; we measured 60 losing order on 2021-11-04.' "$r" 2>/dev/null; then row sync D2-answer-2 pass "verbatim"
    elif grep -q '2021-11-04' docs/work/questions.md 2>/dev/null; then row sync D2-answer-2 pass "not recorded as an answer: raised as a question naming 2021-11-04 (the date belongs to the buffer fix in history — a real conflict)"
    else row sync D2-answer-2 fail "answer 2 neither recorded verbatim nor raised as a question"; fi
    grep -qF 'born in 2020' "$r" 2>/dev/null && row sync D4-conflict fail "the conflicting answer was recorded as an answer" || { grep -q '2020' docs/work/questions.md 2>/dev/null && row sync D4-conflict pass "the 2020 claim is a question" || row sync D4-conflict fail "the 2020 claim is neither answer nor question"; }
    grep -q '2019-03-02' docs/work/questions.md 2>/dev/null && row sync D4-names-evidence pass "the question names the birth date" || row sync D4-names-evidence fail "the question does not name the birth evidence"
    for n in 4096 2021-11-04; do grep -rqF "$n" docs/work/ && row sync "D7-number:$n" pass "present" || row sync "D7-number:$n" n/a "not quoted"; done
    grep -rqF 'düzeltme(sync): saat kayması eşiği 30 saniyeye çekildi' docs/work/ && row sync D8-turkish pass "quoted untranslated" || row sync D8-turkish n/a "the Turkish commit was not quoted"
  else
    grep -rq '^> — [^g]' docs/work/research/ 2>/dev/null && row seed D2-no-answers fail "an answer row without a builder" || row seed D2-no-answers pass "no answer rows"
  fi
  ;;
# ── F5 · a code change under commit_policy: require ─────────────────────────
F5-change-and-commit)
  lint_row change F-lint
  commit_row change F-committed
  # every commit that touches code also touches the docs root
  bad=0
  for c in $(git rev-list seed..HEAD); do
    files=$(git show --format= --name-only "$c")
    code=$(printf '%s\n' "$files" | grep -v "^$ROOT/" | grep -c . || true)
    docs=$(printf '%s\n' "$files" | grep -c "^$ROOT/" || true)
    if [ "$code" -gt 0 ] && [ "$docs" -eq 0 ]; then bad=1; row change F-docs-with-code fail "commit $(git log -1 --format=%h "$c") touched code and no docs"; fi
  done
  [ "$bad" -eq 0 ] && row change F-docs-with-code pass "every commit with code carried a docs change"
  refusals=$(grep -c 'commit_policy: require — nothing lands' "$OUT/transcript.jsonl" || true)
  holds=$(grep -c 'before this session ends' "$OUT/transcript.jsonl" || true)
  row change F-gate-seen pass "gate refusals seen: $refusals · stop holds: $holds (the transcript)"
  grep -rqE '^confirmed:|^verified_by:' "$ROOT/work" "$ROOT/reference" "$ROOT/howto" "$ROOT/explanation" 2>/dev/null \
    && { grep -rhE '^confirmed:|^verified_by:' "$ROOT" | grep -qv 'owner' && row change F-vouching fail "a record names someone who is not the declared maintainer: $(grep -rhE '^confirmed:|^verified_by:' "$ROOT" | head -2 | tr '\n' '|')" || row change F-vouching pass "records name the maintainer only"; } \
    || row change F-vouching pass "no confirmed:/verified_by: written by the session"
  code_changed=$(git diff --name-only seed HEAD | grep -vc "^$ROOT/" || true)
  [ "$code_changed" -gt 0 ] && row change F-the-change pass "$code_changed file(s) outside the docs root changed" || row change F-the-change fail "no code changed"
  ;;
*) row "$TASK" checks n/a "no automatic rows for this task" ;;
esac
printf 'checks: %s rows → %s (%s fail)\n' "$(wc -l < "$AUTO" | tr -d ' ')" "$AUTO" "$(grep -c $'\tfail\t' "$AUTO" || true)"
