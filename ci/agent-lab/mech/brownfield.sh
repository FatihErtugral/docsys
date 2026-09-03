#!/usr/bin/env bash
# mech/brownfield.sh [workdir] — flow 4 without an agent: a project with
# years of history and no documentation. `adopt` (the gate follows lint AND
# refs, D-088), the `seed plan` inventory (mega-commit excluded, features by
# directory+scope, a foreign type word named), refusals (`already covered`,
# `nothing in history names this feature`), one feature's evidence (birth,
# root cause, manifest, dangling `doc:`, the comment block verbatim, tags),
# `--since`, `seed gaps`, a SEED.tsv landed by `seed apply` (idempotent,
# byte-identical), graduation of the seeded block, and a base pulling the
# history through the git connector. Exact strings throughout.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
F=brownfield
WORK=${1:-$(lab_workdir brownfield)}
mkdir -p "$WORK"
assert_outside_repo "$WORK"
"$LAB_DIR/fixtures/gen-brownfield.sh" "$WORK/ledgerkit" >/dev/null
O="$WORK/out"; mkdir -p "$O"
cd "$WORK/ledgerkit"
TODAY=$(date +%F)

say "$F · 1 adopt: the gate's mode follows lint and refs"
docsys adopt > "$O/adopt.out" 2>&1 || check $F adopt FAIL "$(cat "$O/adopt.out")"
expect_in $F adopt-gate-warn "git pre-commit gate: written (warn-mode until lint and refs are clean)" "$O/adopt.out"
expect_in $F adopt-namespace "namespace: ledgerkit" docs/.docmeta.yml
expect_true $F adoption-report "ADOPTION.md" test -f ADOPTION.md
expect_clean $F adopt-lint-clean docs .
(docsys refs --repo . --root docs || true) > "$O/refs.out"
expect_re $F refs-dangling-cite '^ERROR R-076 packages/sync/src/replay\.ts \[sync-replay\]' "$O/refs.out"
git add -A && git commit -qm "adopt docsys" 2>"$O/commit.err" || check $F commit-adopt FAIL "a warn-mode gate must let the adoption land: $(cat "$O/commit.err")"
docsys doctor --repo . --root docs > "$O/doctor.out" 2>&1 || true
expect_in $F doctor-alive "-- pipeline alive" "$O/doctor.out"

say "$F · 2 the inventory"
docsys seed plan --repo . --root docs > "$O/plan.out" 2>&1 || check $F seed-plan FAIL "$(cat "$O/plan.out")"
expect_re $F plan-head '^# head: [0-9a-f]{7}$' "$O/plan.out"
expect_re $F plan-history '^# history: 31 commits \(1 excluded by rule\) · 2019-03-01\.\.' "$O/plan.out"
expect_re $F plan-mega-excluded '^# excluded: [0-9a-f]{7} 2023-06-01 201 files — mega-commit$' "$O/plan.out"
expect_in $F plan-vocab "feat=13 fix=5 chore=4 refactor=2 test=2 docs=1 düzeltme=1 revert=1" "$O/plan.out"
expect_in $F plan-vocab-note "# vocab-note: types outside the tool's English set are counted as work, not fixes: düzeltme — say which are fixes" "$O/plan.out"
expect_in $F plan-feature-sync "# feature sync · directory+manifest+scope · 13 (3) · 2019-03-02..2026-06-14 · uncovered" "$O/plan.out"
expect_in $F plan-feature-core "# feature core · directory+manifest+scope · 7 (2) · 2019-03-01..2025-07-07 · uncovered" "$O/plan.out"
expect_in $F plan-feature-cli "# feature cli · directory+manifest+scope · 6 (1) · 2019-03-01..2026-01-15 · uncovered" "$O/plan.out"
expect_in $F plan-summary "# 3 candidate feature(s), 0 covered, 3 uncovered" "$O/plan.out"
expect_true $F plan-writes-nothing "git status clean after plan" test -z "$(git status --porcelain)"
docsys seed gaps --repo . --root docs > "$O/gaps.json" 2>&1 || true
expect_in $F gaps-sync '{"feature": "sync", "found_by": ["directory", "manifest", "scope"], "paths": ["packages/sync"], "commits": 13, "fixes": 3, "first": "2019-03-02", "last": "2026-06-14", "covered_by": null, "how": null}' "$O/gaps.json"

say "$F · 3 a covered feature and a feature nothing names"
docsys page new reference cli --title "The ledgerkit CLI" --root docs > /dev/null
awk '{ if (index($0, "<!-- opening:") == 1) print "This page is the command surface of the ledgerkit CLI; read it before scripting against it."; else print }' docs/reference/cli.md > "$O/cli.tmp" && mv "$O/cli.tmp" docs/reference/cli.md
printf -- '\n- [[reference/cli|The ledgerkit CLI]] -- the command surface.\n' >> docs/index.md
git add -A && git commit -qm "docs: cli reference" 2>/dev/null
rc=0; docsys seed plan --repo . --root docs --target cli > "$O/cli.out" 2>&1 || rc=$?
expect_true $F covered-refused "exit $rc" test "$rc" != 0
expect_in $F covered-names-page "\`cli\` is already covered by reference/cli.md (page id) — nothing to seed" "$O/cli.out"
docsys seed plan --repo . --root docs > "$O/plan2.out" 2>&1 || true
expect_in $F plan-cli-covered "# feature cli · directory+manifest+scope · 6 (1) · 2019-03-01..2026-01-15 · covered by reference/cli.md (page id)" "$O/plan2.out"
expect_in $F plan-summary-2 "# 3 candidate feature(s), 1 covered, 2 uncovered" "$O/plan2.out"
docsys seed plan --repo . --root docs --target auth > "$O/auth.out" 2>&1 || true
expect_in $F unnamed-escape "# nothing in history names this feature — ask the builder where it lives (a path, a scope, a symbol)" "$O/auth.out"
expect_in $F unnamed-mention "# the string \`auth\` occurs in the diff of 1 commit(s) (git log -S) — a mention in code, not a scope or a path; nothing to seed from" "$O/auth.out"
expect_absent $F unnamed-no-skeleton "# birth" "$O/auth.out"

say "$F · 4 one feature's evidence"
docsys seed plan --repo . --root docs --target sync > "$O/sync.out" 2>&1 || check $F target-sync FAIL "$(cat "$O/sync.out")"
expect_re $F sync-commits '^# commits 15 \(fix 4, revert 1\) · 2019-03-02\.\.2026-06-14 · found by path=15 scope=12 subject=15' "$O/sync.out"
expect_re $F sync-birth '^# birth 2019-03-02 [0-9a-f]{7} — first file added: packages/sync/package\.json$' "$O/sync.out"
expect_in $F sync-root-cause "#   | Root cause: replay accepted frames with clock skew above 30s; the buffer is 4096 entries, not 4000 — 4000 lost 2.3% of events on 2021-11-04." "$O/sync.out"
expect_in $F sync-turkish-subject "düzeltme(sync): saat kayması eşiği 30 saniyeye çekildi" "$O/sync.out"
expect_in $F sync-turkish-body "#   | Sahadaki cihazlar senkronlar arasında 20 saniyeye kadar kayıyor; 60 saniye sıralamayı bozuyordu, 30 saniye ölçümle doğrulandı." "$O/sync.out"
expect_in $F sync-file "# file packages/sync/src/replay.ts · commits 7" "$O/sync.out"
expect_in $F sync-manifest "# manifest packages/sync/package.json · name sync" "$O/sync.out"
expect_in $F sync-cites-dangling "# cites packages/sync/src/replay.ts · doc: sync-replay · dangling" "$O/sync.out"
expect_in $F sync-comment "# comment packages/sync/src/replay.ts@L1-L7 (verbatim)" "$O/sync.out"
expect_in $F sync-comment-line "#   | // Replay keeps the last 4096 entries, not 4000: on 2021-11-04 a 4000-entry" "$O/sync.out"
expect_in $F sync-tag "# tag v1.0.0 · 2021-12-01 (tagged commit) · created 2022-01-10" "$O/sync.out"
expect_absent $F sync-no-vendor "vendor/" "$O/sync.out"
expect_in $F sync-next "# next: /docsys-seed presents this to the builder" "$O/sync.out"
docsys seed plan --repo . --root docs --target sync --since 2024-01-01 > "$O/sync-since.out" 2>&1 || true
expect_re $F sync-since '^# commits 3 \(fix 0, revert 0\) · 2024-08-08\.\.2026-06-14' "$O/sync-since.out"

say "$F · 5 SEED.tsv: the rows land, byte for byte, once"
sha1=$(sha_of . 'feat(sync): first replay loop'); sha2=$(sha_of . 'fix(sync): replay buffer is 4096 entries')
birth=$sha1; fixsha=$sha2
head=$(git rev-parse --short HEAD)
printf '# head: %s\n' "$head" > "$O/SEED.tsv"
printf 'research\tsync\t%s,%s\n' "$sha1" "$sha2" >> "$O/SEED.tsv"
printf 'answer\tsync\tbuilder\tThe window is 30 seconds, not 60:\\nwe measured 60 losing order on 2021-11-04.\n' >> "$O/SEED.tsv"
printf 'journal\t2019-03-02\t%s\tsync born\n' "$birth" >> "$O/SEED.tsv"
printf 'postmortem\tclock-skew\t%s\n' "$fixsha" >> "$O/SEED.tsv"
printf 'debt\t%s\tattribution for the vendor snapshot missing -- deferred: no licence list yet -- repay when: v3\n' "$TODAY" >> "$O/SEED.tsv"
printf 'question\t%s\tIs the 4096 buffer a product limit or a measurement?\n' "$TODAY" >> "$O/SEED.tsv"
docsys seed apply --plan "$O/SEED.tsv" --repo . --root docs > "$O/apply.out" 2>&1 || check $F seed-apply FAIL "$(cat "$O/apply.out")"
r=docs/work/research/sync.md
expect_in $F research-id "id: sync" $r
expect_in $F research-active "status: active" $r
expect_in $F research-seeded "seeded: true" $r
expect_in $F research-covers "covers: [scope:sync]" $r
expect_in $F research-sources "sources: [git:$sha1, git:$sha2]" $r
expect_in $F research-headings "## Why no decision" $r
expect_in $F answer-quoted "> The window is 30 seconds, not 60:" $r
expect_in $F answer-second-line "> we measured 60 losing order on 2021-11-04." $r
expect_in $F answer-attributed "> — builder, $TODAY" $r
expect_in $F journal-entry "## 2019-03-02 - sync born" docs/work/journal.md
expect_in $F journal-provenance "- git: $birth" docs/work/journal.md
expect_true $F journal-order "the 2019 entry sits below today's adoption entry (R-104)" \
  sh -c "awk '/^## /{print}' docs/work/journal.md | head -1 | grep -q '^## $TODAY' && awk '/^## /{print}' docs/work/journal.md | tail -1 | grep -q '^## 2019-03-02 - sync born'"
pm=docs/work/postmortems/clock-skew.md
expect_in $F postmortem-subject "> fix(sync): replay buffer is 4096 entries" $pm
expect_in $F postmortem-body "> Root cause: replay accepted frames with clock skew above 30s; the buffer is 4096 entries, not 4000 — 4000 lost 2.3% of events on 2021-11-04." $pm
expect_in $F postmortem-provenance "> — git:$fixsha" $pm
expect_in $F debt-item "- [ ] $TODAY attribution for the vendor snapshot missing -- deferred: no licence list yet -- repay when: v3" docs/work/debt.md
expect_in $F question-item "- [ ] $TODAY Is the 4096 buffer a product limit or a measurement?" docs/work/questions.md
expect_clean $F after-apply docs .
git add docs && git commit -qm "seed: sync" 2>"$O/commit.err" || check $F commit-seed FAIL "$(cat "$O/commit.err")"
expect_true $F seed-tsv-not-committed "SEED.tsv stays outside" test -z "$(git ls-files | grep SEED.tsv)"
cp -r docs "$O/docs.first"
docsys seed apply --plan "$O/SEED.tsv" --repo . --root docs > "$O/apply2.out" 2>&1 || check $F seed-apply-2 FAIL "$(cat "$O/apply2.out")"
expect_true $F apply-idempotent-lines "every line says already ($(tr '\n' '|' < "$O/apply2.out"))" test "$(grep -vc 'already' "$O/apply2.out")" = 0
expect_true $F apply-idempotent-bytes "docs/ byte-identical after the second apply" diff -rq docs "$O/docs.first"
expect_true $F apply-idempotent-tree "nothing to commit" test -z "$(git status --porcelain)"
rc=0; docsys seed plan --repo . --root docs --target sync > "$O/sync2.out" 2>&1 || rc=$?
expect_in $F seeded-now-covered "\`sync\` is already covered by work/research/sync.md (covers: scope)" "$O/sync2.out"

say "$F · 6 the seeded block graduates, verbatim"
docsys page new explanation sync-replay --title "Why the sync window is 30 seconds" --root docs >/dev/null
awk '{ if (index($0, "<!-- opening:") == 1) print "This page explains why the sync window is 30 seconds and the replay buffer 4096 entries; read it before changing either number."; else print }' docs/explanation/sync-replay.md > "$O/sr.tmp" && mv "$O/sr.tmp" docs/explanation/sync-replay.md
printf -- '- [[explanation/sync-replay|Why the sync window is 30 seconds]] -- the numbers and their measurements.\n' >> docs/index.md
git add -A && git commit -qm "docs: sync-replay prepared" 2>/dev/null
docsys graduate plan work/research/sync.md --root docs > "$O/g.plan"
awk -F'\t' '
  /^# block / { n = $0; sub(/^# block /, "", n); sub(/ .*/, "", n); h = $0; sub(/.*· "/, "", h); sub(/"$/, "", h); heading[n] = h; print; next }
  /^[0-9]+\tkeep$/ { if (heading[$1] == "## Learned") print $1 "\tmove:explanation/sync-replay"; else print; next }
  { print }
' "$O/g.plan" > "$O/g.plan.f" && mv "$O/g.plan.f" "$O/g.plan"
docsys graduate apply --plan "$O/g.plan" --root docs > "$O/gapply.out" 2>&1 || check $F graduate-seeded FAIL "$(cat "$O/gapply.out")"
expect_in $F seeded-moved "Moved to [[explanation/sync-replay|sync-replay]]." $r
expect_in $F quote-arrived "> we measured 60 losing order on 2021-11-04." docs/explanation/sync-replay.md
expect_in $F attribution-arrived "> — builder, $TODAY" docs/explanation/sync-replay.md
expect_in $F research-still-active "status: active" $r
expect_in $F cite-now-resolves "id: sync-replay" docs/explanation/sync-replay.md
(docsys refs --repo . --root docs || true) > "$O/refs2.out"
expect_absent $F dangling-cite-gone "R-076" "$O/refs2.out"
expect_clean $F after-graduation docs .
git add -A && git commit -qm "graduate: sync learned" 2>"$O/commit.err" || check $F commit-graduate FAIL "$(cat "$O/commit.err")"
docsys adopt > "$O/adopt2.out" 2>&1 || true
expect_in $F gate-hardened "git pre-commit gate: hardened" "$O/adopt2.out"

say "$F · 7 a base learns from this repository through the connector"
"$LAB_DIR/fixtures/gen-kb.sh" "$WORK/kb" >/dev/null
cd "$WORK/kb"
docsys inbox pull "$WORK/ledgerkit" --since 2019-01-01 --limit 10 --root . > "$O/pull.out" 2>&1 || true
expect_true $F pull-ten "exactly 10 captured" test "$(grep -c '^captured: raw/inbox/' "$O/pull.out")" = 10
expect_absent $F pull-no-bookkeeping "bump-readme" "$O/pull.out"
expect_true $F pull-keeps-docs-with-body "the idempotency explanation lands" grep -q 'why-replay-is-idempotent' "$O/pull.out"
mega=$(grep -l 'vendor snapshot' raw/inbox/*.md | head -1)
expect_true $F pull-mega-lands "the 201-file commit lands as a record (a finding, not a failure)" test -n "$mega"
[ -n "$mega" ] && expect_true $F pull-mega-201-files "201 file lines" test "$(grep -c '^- vendor/left-pad/' "$mega")" = 201
docsys inbox pull "$WORK/ledgerkit" --since 2019-01-01 --limit 10 --root . > "$O/pull2.out" 2>&1 || true
expect_true $F pull-idempotent "10 already captured" test "$(grep -c '^already captured: ' "$O/pull2.out")" = 10
docsys inbox pull "$WORK/ledgerkit" --since 2019-01-01 --limit 10 --all --root . > "$O/pull3.out" 2>&1 || true
expect_true $F pull-all-adds-bookkeeping "bookkeeping commits with --all" test "$(grep -c '^captured: raw/inbox/' "$O/pull3.out")" -ge 3
docsys inbox pull "$WORK/ledgerkit" --since 2026-01-01 --as lk --root . > "$O/pull4.out" 2>&1 || true
expect_true $F pull-as-namespace "lk@ records" sh -c "grep -l '^source_id: lk@' raw/inbox/*.md >/dev/null"
docsys status --root . > "$O/status.out"
expect_in $F status-counts "inbox: 26 note(s), oldest 2023-01-25" "$O/status.out"
expect_clean $F kb-clean .
summary $F
