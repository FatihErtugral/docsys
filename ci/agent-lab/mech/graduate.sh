#!/usr/bin/env bash
# mech/graduate.sh [workdir] — flow 2 without an agent: `graduate plan` and
# `graduate apply` against the answer key (R-049), byte-exact arrival
# (R-090), the `link:` disposition (R-093), the dirty-tree refusal (R-097),
# drift (re-plan), a missing destination (R-099), and the file-level
# transition (R-081, R-082). Every expectation is an exact string.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
F=graduate
WORK=${1:-$(lab_workdir graduate)}
mkdir -p "$WORK"
assert_outside_repo "$WORK"
"$LAB_DIR/fixtures/gen-project.sh" "$WORK/proj" >/dev/null
cd "$WORK/proj"
O="$WORK/out"; mkdir -p "$O"
TODAY=$(date +%F)
KEY="$LAB_DIR/fixtures/project/expected-dispositions.tsv"

# fill_plan <plan> <source-rel> — the second column from the answer key.
fill_plan() {
  awk -F'\t' -v src="$2" -v key="$KEY" '
    BEGIN { while ((getline line < key) > 0) { n = split(line, f, "\t"); if (n == 3 && f[1] == src) disp[f[2]] = f[3] } }
    /^# block / { n = $0; sub(/^# block /, "", n); sub(/ .*/, "", n); h = $0; sub(/.*· "/, "", h); sub(/"$/, "", h); heading[n] = h; print; next }
    /^[0-9]+\tkeep$/ { d = disp[heading[$1]]; if (d == "") d = "keep"; print $1 "\t" d; next }
    { print }
  ' "$1" > "$1.filled" && mv "$1.filled" "$1"
}
# appended_lines <before-file> <after-file> — the lines the destination gained, non-empty, no headings.
appended_lines() {
  tail -c +"$(( $(wc -c < "$1") + 1 ))" "$2" | grep -v '^#' | grep -v '^[[:space:]]*$' || true
}
# arrived_byte_exact <flow> <name> <source-before> <dest-before> <dest-after>
arrived_byte_exact() {
  local bad=0 line
  while IFS= read -r line; do
    grep -qFx -- "$line" "$3" || { bad=1; check "$1" "$2" FAIL "not in the source before graduation: $line"; break; }
  done < <(appended_lines "$4" "$5")
  [ "$bad" -eq 0 ] && check "$1" "$2" PASS "every appended line is a line of the source ($(appended_lines "$4" "$5" | wc -l | tr -d ' ') lines)"
}

say "$F · 1 plan: blocks, R-048 flags, keep rows"
docsys graduate plan work/features/cart-key.md --root docs > "$WORK/cart-key.plan"
expect_in $F plan-source "# source: work/features/cart-key.md" "$WORK/cart-key.plan"
expect_true $F plan-five-blocks "5 block lines" test "$(grep -c '^# block ' "$WORK/cart-key.plan")" = 5
expect_re $F plan-r048-context '^# block 0 · L[0-9]+-L[0-9]+ · fnv:[0-9a-f]{16} · heading stays \(R-048\) · "## Context"$' "$WORK/cart-key.plan"
expect_re $F plan-notes-no-flag '^# block 4 · L[0-9]+-L[0-9]+ · fnv:[0-9a-f]{16} · "## Notes"$' "$WORK/cart-key.plan"
expect_true $F plan-keep-rows "5 keep rows" test "$(grep -cE '^[0-9]+	keep$' "$WORK/cart-key.plan")" = 5
fill_plan "$WORK/cart-key.plan" work/features/cart-key.md
expect_in $F filled-link "4	link:reference/keys" "$WORK/cart-key.plan"
expect_in $F filled-move "1	move:explanation/cart-key-decision" "$WORK/cart-key.plan"

say "$F · 2 a dirty tree is refused (R-097)"
touch docs/scratch
rc=0; docsys graduate apply --plan "$WORK/cart-key.plan" --root docs > $O/apply.out 2>&1 || rc=$?
expect_true $F dirty-refused "exit $rc" test "$rc" != 0
expect_in $F dirty-names-rule "(R-097)" $O/apply.out
rm docs/scratch

say "$F · 3 apply: the feature lands where the table says"
cp docs/work/features/cart-key.md "$WORK/cart-key.before"
for d in reference/cart-key-contract explanation/cart-key-decision reference/keys; do cp "docs/$d.md" "$WORK/$(basename $d).before"; done
docsys graduate apply --plan "$WORK/cart-key.plan" --root docs > $O/apply.out 2>&1 || check $F apply-1 FAIL "$(cat $O/apply.out)"
src=docs/work/features/cart-key.md
expect_in $F moved-decision "Moved to [[explanation/cart-key-decision|cart-key-decision]]." $src
expect_in $F moved-contract "Moved to [[reference/cart-key-contract|cart-key-contract]]." $src
expect_in $F linked-notes "Already documented: [[reference/keys|keys]]." $src
expect_in $F heading-stays "## Decision" $src
expect_absent $F sentence-left "The key is the SHA of cart-id + day." $src
expect_in $F sentence-arrived "The key is the SHA of cart-id + day." docs/explanation/cart-key-decision.md
expect_in $F rejected-arrived "Keying by session id: the second tab." docs/explanation/cart-key-decision.md
expect_in $F contract-arrived 'returns today'"'"'s document for that cart' docs/reference/cart-key-contract.md
expect_in $F graduated-to "graduated_to: [cart-key-decision, cart-key-contract, keys]" $src
expect_true $F keys-untouched "reference/keys.md unchanged by a link:" cmp -s docs/reference/keys.md "$WORK/keys.before"
expect_in $F dest-updated "updated: $TODAY" docs/explanation/cart-key-decision.md
arrived_byte_exact $F byte-exact-decision "$WORK/cart-key.before" "$WORK/cart-key-decision.before" docs/explanation/cart-key-decision.md
arrived_byte_exact $F byte-exact-contract "$WORK/cart-key.before" "$WORK/cart-key-contract.before" docs/reference/cart-key-contract.md
expect_in $F status-unchanged "status: done" $src
expect_clean $F after-feature docs .
git add -A && git commit -qm "graduate: cart-key" 2>$O/commit.err || check $F commit-1 FAIL "$(cat $O/commit.err)"

say "$F · 4 apply: the postmortem to three pages, the research to one"
docsys graduate plan work/postmortems/cache-stampede.md --root docs > "$WORK/pm.plan"
fill_plan "$WORK/pm.plan" work/postmortems/cache-stampede.md
cp docs/work/postmortems/cache-stampede.md "$WORK/pm.before"
for d in explanation/cache-stampede-cause reference/cache-stampede-invariant howto/cache-stampede-runbook; do cp "docs/$d.md" "$WORK/$(basename $d).before"; done
docsys graduate apply --plan "$WORK/pm.plan" --root docs > $O/apply.out 2>&1 || check $F apply-2 FAIL "$(cat $O/apply.out)"
src=docs/work/postmortems/cache-stampede.md
expect_in $F pm-graduated-to "graduated_to: [cache-stampede-cause, cache-stampede-invariant, cache-stampede-runbook]" $src
expect_in $F pm-kept "every key had been written at the same second" $src
expect_in $F pm-invariant-arrived 'test_expiry_jitter_spreads_a_burst' docs/reference/cache-stampede-invariant.md
expect_in $F pm-runbook-arrived "1. Stop the warm-up job." docs/howto/cache-stampede-runbook.md
arrived_byte_exact $F byte-exact-cause "$WORK/pm.before" "$WORK/cache-stampede-cause.before" docs/explanation/cache-stampede-cause.md
arrived_byte_exact $F byte-exact-runbook "$WORK/pm.before" "$WORK/cache-stampede-runbook.before" docs/howto/cache-stampede-runbook.md
git add -A && git commit -qm "graduate: cache-stampede" 2>"$O/commit.err" || check $F commit-pm FAIL "$(cat "$O/commit.err")"
docsys graduate plan work/research/retry-budget.md --root docs > "$WORK/rb.plan"
fill_plan "$WORK/rb.plan" work/research/retry-budget.md
cp docs/work/research/retry-budget.md "$WORK/rb.before"
cp docs/explanation/retry-budget-findings.md "$WORK/retry-budget-findings.before"
docsys graduate apply --plan "$WORK/rb.plan" --root docs > $O/apply.out 2>&1 || check $F apply-3 FAIL "$(cat $O/apply.out)"
src=docs/work/research/retry-budget.md
expect_in $F rb-graduated-to "graduated_to: [retry-budget-findings]" $src
expect_in $F rb-question-kept "Should a tenant have a retry budget" $src
expect_true $F rb-three-moved "three Moved lines" test "$(grep -c 'Moved to \[\[explanation/retry-budget-findings|retry-budget-findings\]\]\.' $src)" = 3
arrived_byte_exact $F byte-exact-findings "$WORK/rb.before" "$WORK/retry-budget-findings.before" docs/explanation/retry-budget-findings.md
expect_clean $F after-three docs .
git add -A && git commit -qm "graduate: cache-stampede, retry-budget" 2>$O/commit.err || check $F commit-2 FAIL "$(cat $O/commit.err)"

say "$F · 5 drift and a missing destination are refused"
docsys graduate plan work/features/cart-key.md --root docs > "$WORK/drift.plan"
awk -F'\t' '/^0\tkeep$/ { print "0\tmove:explanation/cart-key-decision"; next } { print }' "$WORK/drift.plan" > "$WORK/drift.plan.f" && mv "$WORK/drift.plan.f" "$WORK/drift.plan"
printf 'A line added after the plan was written.\n' >> docs/work/features/cart-key.md
git add -A && git commit -qm "cart-key: drift" 2>/dev/null
rc=0; docsys graduate apply --plan "$WORK/drift.plan" --root docs > $O/apply.out 2>&1 || rc=$?
expect_true $F drift-refused "exit $rc" test "$rc" != 0
expect_in $F drift-says-replan "re-plan" $O/apply.out
docsys graduate plan work/features/cart-key.md --root docs > "$WORK/nowhere.plan"
awk -F'\t' '/^0\tkeep$/ { print "0\tmove:reference/nowhere"; next } { print }' "$WORK/nowhere.plan" > "$WORK/nowhere.plan.f" && mv "$WORK/nowhere.plan.f" "$WORK/nowhere.plan"
rc=0; docsys graduate apply --plan "$WORK/nowhere.plan" --root docs > $O/apply.out 2>&1 || rc=$?
expect_true $F nowhere-refused "exit $rc" test "$rc" != 0
expect_in $F nowhere-names-rule "R-099" $O/apply.out
expect_in $F source-intact "## Context" docs/work/features/cart-key.md

say "$F · 6 the file-level transition: graduated needs the owner's word, and stays still"
src=docs/work/features/cart-key.md
replace_line $src "status: done" "status: graduated"
expect_clean $F graduated-confirmed docs .
grep -v '^confirmed:' $src > "$src.tmp" && mv "$src.tmp" $src
lint_to docs $O/lint.out
expect_re $F graduated-unconfirmed '^WARN R-081 work/features/cart-key\.md \[confirmed\]' $O/lint.out
git checkout -q -- $src
replace_line $src "status: done" "status: graduated"
git add -A && git commit -qm "cart-key: graduated" 2>$O/commit.err || check $F commit-3 FAIL "$(cat $O/commit.err)"
expect_clean $F graduated-committed docs .
printf 'Edited after graduation.\n' >> $src
lint_to docs $O/lint.out
expect_re $F graduated-edited '^ERROR R-082 work/features/cart-key\.md \[content\]' $O/lint.out
git checkout -q -- $src
summary $F
