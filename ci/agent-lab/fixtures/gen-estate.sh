#!/usr/bin/env bash
# gen-estate.sh <dir> — three projects and a stranger, for the learning flow.
#
# `relay`, `ledger` and `gateway` are project trees (docs/ with a reference
# page and a howto, an export manifest, dated commits WITH bodies — the git
# connector keeps those), plus bookkeeping commits (docs-only, no body) the
# connector skips. `relay` carries exactly 2 commits worth reading and 3
# bookkeeping ones, so `--limit` arithmetic is checkable. `notebook` is
# another knowledge base: `docsys assistant --projects <dir>` must skip it.
# Nothing here is a hub: the hub is what `docsys assistant` builds.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
DIR=${1:?usage: gen-estate.sh <dir>}
[ -e "$DIR" ] && fail "$DIR exists"
mkdir -p "$DIR"

provider() { # provider <name> <ref-id> <ref-title> <ref-body> <howto-id> <howto-title> <date>
  local name=$1 rid=$2 rtitle=$3 rbody=$4 hid=$5 htitle=$6 date=$7
  local p="$DIR/$name"
  lab_git_init "$p"
  mkdir -p "$p/docs/reference" "$p/docs/howto" "$p/src"
  printf 'pub fn %s() {}\n' "$name" > "$p/src/lib.rs"
  cat > "$p/docs/.docmeta.yml" <<MD
spec: docsys/0.4
profile: project
default_content_language: en
namespace: $name
MD
  printf '# %s\n\n- [[reference/%s|%s]] -- the promise.\n- [[howto/%s|%s]] -- the procedure.\n' "$name" "$rid" "$rtitle" "$hid" "$htitle" > "$p/docs/index.md"
  cat > "$p/docs/reference/$rid.md" <<MD
---
id: $rid
type: reference
updated: $date
---
# $rtitle

This page states what $name promises to its callers; read it before depending on $name.

$rbody
MD
  cat > "$p/docs/howto/$hid.md" <<MD
---
id: $hid
type: howto
updated: $date
---
# $htitle

This page is the procedure for running $name locally; read it before the first run.

1. Build it.
2. Start it with the default configuration.
3. Check the health endpoint answers.
MD
  ( cd "$p" && docsys export manifest --root docs --out docs/manifest.docsys >/dev/null 2>&1 )
  dated_commit "$p" "$date" "$name: first page and procedure" "The first documented promise of $name, and why it exists: callers kept guessing."
}

provider relay retry-policy "Retry policy" "Four attempts, exponential backoff starting at 200 ms, then a dead letter. The fourth attempt is the last: the caller sees the dead letter, never a fifth try." run-relay "Run relay locally" 2026-05-10
provider ledger transfer-semantics "Transfer semantics" "A transfer is refused, never partially applied: both legs commit or neither does. The refusal carries the reason code the caller must show the person." run-ledger "Run ledger locally" 2026-05-12
provider gateway rate-limits "Rate limits" "1000 requests per minute per key, counted in a sliding window; the 1001st receives 429 with a Retry-After of the window's remainder." run-gateway "Run gateway locally" 2026-05-14

# relay: one more commit worth reading (a body), then three bookkeeping commits
printf 'pub fn relay() {}\npub fn backoff(attempt: u32) -> u64 { 200 * 2u64.pow(attempt) }\n' > "$DIR/relay/src/lib.rs"
dated_commit "$DIR/relay" 2026-06-02 "relay: backoff doubles from 200 ms" "Root cause of the May incident: a fixed 200 ms retry hammered the dependency; doubling gives it room. Measured: 4 attempts finish under 3.2 s."
for i in 1 2 3; do
  printf "\nEdited %s.\n" "$i" >> "$DIR/relay/docs/howto/run-relay.md"
  bump_updated "$DIR/relay/docs/howto/run-relay.md" "2026-06-0$((i + 2))"
  dated_commit "$DIR/relay" "2026-06-0$((i + 2))" "docs: touch run-relay ($i)"
done
# ledger: one bookkeeping commit
printf '\nSee also the reference.\n' >> "$DIR/ledger/docs/howto/run-ledger.md"
bump_updated "$DIR/ledger/docs/howto/run-ledger.md" 2026-06-10
dated_commit "$DIR/ledger" 2026-06-10 "docs: cross-link"
# gateway: a code commit with a body
printf 'pub fn gateway() {}\npub const WINDOW_SECS: u64 = 60;\n' > "$DIR/gateway/src/lib.rs"
dated_commit "$DIR/gateway" 2026-06-12 "gateway: window is 60 seconds" "Chosen over 1 second buckets: the sliding window is what the SLA text says, and the bucket variant let bursts through at bucket edges."

# a stranger: another knowledge base one level under the same directory
lab_git_init "$DIR/notebook"
( cd "$DIR/notebook" && DOCSYS_TODAY=2026-05-01 docsys init --profile knowledge-base --root . >/dev/null )
dated_commit "$DIR/notebook" 2026-05-01 "notebook: a base of its own"

for p in relay ledger gateway; do
  lint_clean "$DIR/$p/docs" "$DIR/$p" || fail "$p is not clean: $(docsys lint --root "$DIR/$p/docs" --repo "$DIR/$p" | grep -E '^(ERROR|WARN)' | head -3)"
done
printf 'gen-estate: %s (relay %s, ledger %s, gateway %s)\n' "$DIR" \
  "$(git -C "$DIR/relay" rev-parse --short HEAD)" "$(git -C "$DIR/ledger" rev-parse --short HEAD)" "$(git -C "$DIR/gateway" rev-parse --short HEAD)"
