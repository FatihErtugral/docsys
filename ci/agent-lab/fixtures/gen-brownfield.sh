#!/usr/bin/env bash
# gen-brownfield.sh <dir> — `ledgerkit`, an invented project with years of
# history and no documentation: the brownfield case.
#
# 2019-03-01 → 2026-06-14, dated commits, three scopes with manifests
# (`packages/core`, `packages/sync`, `apps/cli`), conventional subjects with
# bodies (root causes, measurements, numbers), one Turkish subject with a
# Turkish body, bookkeeping commits (README only, no body) the connector
# skips, a docs-only commit WITH a body it keeps, one 201-file vendor
# snapshot (a mega-commit `seed` excludes by rule), a code comment block
# that says WHY (with two numbers) and a dangling `doc:` citation, three
# tags. `auth` appears only inside commit bodies — never a subject or a
# path — so `seed plan --target auth` has nothing to find. No docsys here:
# adoption is the flow's first step, not the fixture's.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
DIR=${1:?usage: gen-brownfield.sh <dir>}
[ -e "$DIR" ] && fail "$DIR exists"
lab_git_init "$DIR"
cd "$DIR"

# c <date> <subject> <body-or-empty> <file>… — touch the files (append one
# deterministic line), commit with the date, the subject and the body.
c() {
  local date=$1 subject=$2 body=$3; shift 3
  local f
  for f in "$@"; do
    mkdir -p "$(dirname "$f")"
    printf '// %s — %s\n' "$date" "$subject" >> "$f"
  done
  dated_commit . "$date" "$subject" "$body"
}

mkdir -p packages/core/src apps/cli/src
printf '# ledgerkit\n\nAn offline-first ledger for field devices. Invented for the docsys agent lab.\n' > README.md
printf '{\n  "name": "core",\n  "version": "0.0.1",\n  "private": true\n}\n' > packages/core/package.json
printf '{\n  "name": "cli",\n  "version": "0.0.1",\n  "private": true\n}\n' > apps/cli/package.json
printf 'node_modules/\n' > .gitignore
printf 'export function open(path: string) { return { path } }\n' > packages/core/src/index.ts
printf 'import { open } from "../../../packages/core/src/index"\nconsole.log(open(process.argv[2]))\n' > apps/cli/src/main.ts
dated_commit . 2019-03-01 "chore: initial commit"

mkdir -p packages/sync/src
printf '{\n  "name": "sync",\n  "version": "0.0.1",\n  "private": true\n}\n' > packages/sync/package.json
cat > packages/sync/src/replay.ts <<'TS'
// Replay keeps the last 4096 entries, not 4000: on 2021-11-04 a 4000-entry
// buffer lost 2.3% of events while the clock skew window was 30 seconds.
// The window is 30 seconds because the field devices drift up to 20 seconds
// between syncs, and the trains lose the network for the rest of the trip.
// Nothing below this comment changes the two numbers without a new
// measurement; replay.test.ts pins both.
// doc: sync-replay
export const BUFFER = 4096
export const SKEW_WINDOW_SECONDS = 30
export function replay(entries: unknown[]) { return entries.slice(-BUFFER) }
TS
dated_commit . 2019-03-02 "feat(sync): first replay loop" "Replay applies the entries a device recorded offline, in order, when the network returns."
c 2019-04-11 "feat(core): journal file format" "One line per entry, append only; auth stays inside core until the token format settles." packages/core/src/journal.ts
c 2019-05-20 "feat(cli): open and dump a ledger" "" apps/cli/src/main.ts
c 2019-06-15 "feat(sync): sequence numbers on every entry" "Every entry carries its own sequence number so a batch can be applied twice without harm." packages/sync/src/replay.ts
c 2019-08-30 "fix(core): journal truncation on power loss" "Root cause: the last line was written before fsync; now fsync precedes the length update." packages/core/src/journal.ts
dated_tag . 2019-09-01 v0.1.0
c 2019-11-02 "chore: bump readme" "" README.md
c 2020-02-02 "feat(core): tokens for device identity" "The auth module will move out of core once the token format is stable; today it is one file." packages/core/src/tokens.ts
c 2020-05-14 "feat(sync): conflict rule — last writer per key" "Per key, the entry with the higher sequence wins; ties go to the device with the lower id, which is deterministic and documented nowhere else." packages/sync/src/conflicts.ts
c 2020-09-09 "test(sync): replay applies a batch twice as a no-op" "" packages/sync/src/replay.test.ts
c 2021-01-19 "feat(cli): sync command" "" apps/cli/src/sync.ts
c 2021-03-03 "refactor(sync): split the replay loop from the conflict rule" "" packages/sync/src/replay.ts packages/sync/src/conflicts.ts
c 2021-06-21 "feat(sync): clock skew window" "Entries stamped more than 60 seconds ahead of the receiver are held until the next sync." packages/sync/src/skew.ts
c 2021-11-06 "fix(sync): replay buffer is 4096 entries" "Root cause: replay accepted frames with clock skew above 30s; the buffer is 4096 entries, not 4000 — 4000 lost 2.3% of events on 2021-11-04." packages/sync/src/replay.ts packages/sync/src/replay.test.ts
c 2021-12-01 "chore: bump readme" "" README.md
dated_tag . 2022-01-10 v1.0.0
c 2022-03-15 "düzeltme(sync): saat kayması eşiği 30 saniyeye çekildi" "Sahadaki cihazlar senkronlar arasında 20 saniyeye kadar kayıyor; 60 saniye sıralamayı bozuyordu, 30 saniye ölçümle doğrulandı." packages/sync/src/skew.ts
c 2022-05-05 "feat(core): compaction of old journal segments" "" packages/core/src/compact.ts
c 2022-08-18 "fix(cli): exit code on a failed sync" "" apps/cli/src/sync.ts
c 2022-10-10 "revert: feat(sync): batch acknowledgements" "Reverts the acknowledgement batching: the field devices ran out of memory holding unacknowledged batches." packages/sync/src/replay.ts
c 2023-01-25 "feat(cli): offline replay from a file" "The CLI replays a captured file when there is no network at all: the trains." apps/cli/src/replay.ts
c 2023-06-01 "chore: vendor snapshot" "" $(for i in $(seq 1 201); do printf 'vendor/left-pad/%03d.js ' "$i"; done)
c 2023-09-12 "fix(sync): duplicate entries after a resumed sync" "Root cause: the resume cursor pointed at the last applied entry, not past it." packages/sync/src/replay.ts
c 2024-02-14 "feat(core): checksums per segment" "" packages/core/src/checksum.ts
c 2024-05-20 "docs(readme): why replay is idempotent" "Replay is idempotent because every entry carries its own sequence number; applying a batch twice is a no-op, and the CLI relies on that when a train regains the network." README.md
c 2024-08-08 "test(sync): skew window holds entries 30 seconds ahead" "" packages/sync/src/skew.test.ts
c 2024-11-30 "chore: bump readme" "" README.md
dated_tag . 2025-02-02 v2.0.0
c 2025-03-03 "feat(sync): resumable sync cursor" "The cursor is the sequence number of the last applied entry plus one; see the 2023 duplicate-entries fix." packages/sync/src/cursor.ts
c 2025-07-07 "fix(core): compaction kept a stale checksum" "" packages/core/src/compact.ts packages/core/src/checksum.ts
c 2026-01-15 "feat(cli): progress output during replay" "" apps/cli/src/replay.ts
c 2026-06-14 "refactor(sync): typed entries" "" packages/sync/src/replay.ts packages/sync/src/conflicts.ts

printf 'gen-brownfield: %s at %s, %s commits\n' "$DIR" "$(git rev-parse --short HEAD)" "$(git rev-list --count HEAD)"
