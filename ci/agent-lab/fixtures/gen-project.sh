#!/usr/bin/env bash
# gen-project.sh <dir> — a project at the start of the graduation flow.
#
# Adopted today, three finished work files (a feature, a postmortem, a
# research page — every one `status: done` and confirmed by the owner),
# the destination pages already prepared with `docsys page new` and an
# authored opening, all routed from index.md, and one pre-existing page
# (`reference/keys`) whose text the feature's `## Notes` duplicates — the
# `link:` case. fixtures/project/expected-dispositions.tsv is the R-049
# answer key. Lint is clean at the tag `seed`.
source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
lab_binary
DIR=${1:?usage: gen-project.sh <dir>}
[ -e "$DIR" ] && fail "$DIR exists"
TODAY=$(date +%F)

lab_git_init "$DIR"
cd "$DIR"
printf '# cartd\n\nA cart service. Invented for the docsys agent lab.\n' > README.md
mkdir -p src && printf 'pub fn key(cart: &str, day: &str) -> String { format!("{cart}:{day}") }\n' > src/lib.rs
docsys adopt >/dev/null

# destinations first (R-099), each with an authored opening
new_page() { # new_page <type> <id> <title> <opening>
  docsys page new "$1" "$2" --title "$3" --root docs >/dev/null
  awk -v opening="$4" '{ if (index($0, "<!-- opening:") == 1) print opening; else print }' "docs/$1/$2.md" > page.tmp && mv page.tmp "docs/$1/$2.md"
}
new_page reference keys "Keys" "This page is the canonical form every cache key in cartd is derived from; read it before computing or comparing a key."
printf '\nKeys are SHA-256 over the canonical form: fields sorted by name, UTF-8, no whitespace between them.\n' >> docs/reference/keys.md
new_page reference cart-key-contract "Cart key contract" "This page is the contract of the cart key as the API exposes it; read it before calling the cart endpoints."
new_page explanation cart-key-decision "Why the cart key is per day" "This page explains why a cart is keyed per day and what was rejected; read it before proposing another key."
new_page explanation cache-stampede-cause "Why the cache stampeded" "This page explains the cause of the cache stampede of the launch week; read it before changing cache expiry."
new_page reference cache-stampede-invariant "Cache expiry invariant" "This page states the invariant that keeps the cache from stampeding, and the test that guards it; read it before touching expiry."
new_page howto cache-stampede-runbook "Recover from a cache stampede" "This page is the runbook for a cache stampede in progress; read it when the origin's latency climbs with the hit rate falling."
new_page explanation retry-budget-findings "Retry budget: what was tried" "This page explains what was learned about a per-tenant retry budget and why nothing was decided; read it before reopening the question."
cat >> docs/index.md <<'MD'

- [[reference/keys|Keys]] -- the canonical form under every cache key.
- [[reference/cart-key-contract|Cart key contract]] -- what the API exposes.
- [[explanation/cart-key-decision|Why the cart key is per day]] -- the decision and what was rejected.
- [[explanation/cache-stampede-cause|Why the cache stampeded]] -- the launch-week incident's cause.
- [[reference/cache-stampede-invariant|Cache expiry invariant]] -- the invariant and its test.
- [[howto/cache-stampede-runbook|Recover from a cache stampede]] -- the runbook.
- [[explanation/retry-budget-findings|Retry budget: what was tried]] -- findings without a decision.
MD

mkdir -p docs/work/features docs/work/postmortems docs/work/research
cat > docs/work/features/cart-key.md <<MD
---
id: cart-key
status: done
confirmed: owner, $TODAY
updated: $TODAY
---
## Context

The cart service keyed its cache by session id, which broke the day a customer opened the shop in a second tab and saw an empty cart.

## Decision

The key is the SHA of cart-id + day. A cart is one document per calendar day, whatever the tab, whatever the session.

## Contract surface

\`GET /cart/{id}\` returns today's document for that cart; a key from yesterday is never served, it is recomputed.

## Rejected alternatives

Keying by session id: the second tab. Keying by cart id alone: a cart that never expires and a cache that never shrinks.

## Notes

Keys are SHA-256 over the canonical form: fields sorted by name, UTF-8, no whitespace between them.
MD
cat > docs/work/postmortems/cache-stampede.md <<MD
---
id: cache-stampede
status: done
confirmed: owner, $TODAY
updated: $TODAY
---
## What happened

On launch day every cart key expired at the same second, because every key had been written at the same second by the warm-up job.

## Root cause

Expiry was a constant added to the write time; a warm-up that writes every key in one pass gives every key one expiry.

## Recurrence

Invariant: no two keys written in the same second share an expiry — the jitter is at least 10% of the TTL. Guarded by \`test_expiry_jitter_spreads_a_burst\`.

## Lesson

1. Stop the warm-up job.
2. Raise the TTL of the keys still alive by 30 minutes, once.
3. Restart the warm-up with jitter enabled and watch the origin's p99 for ten minutes.
MD
cat > docs/work/research/retry-budget.md <<MD
---
id: retry-budget
status: done
confirmed: owner, $TODAY
updated: $TODAY
---
## Question

Should a tenant have a retry budget, so that one tenant's storm cannot spend the whole pool's retries?

## Tried

A fixed budget of 100 retries per minute per tenant: the two largest tenants hit it on a normal Monday. A budget proportional to the tenant's quota: fair, but it moved the storm to the tenants with the largest quotas.

## Learned

The storms come from one endpoint, not from one tenant; a per-endpoint budget would have caught every incident of the last quarter.

## Why no decision

The per-endpoint budget needs the endpoint in the retry path, which the client does not send today; the question is closed until the client is versioned.
MD
lint_clean docs . || fail "the project is not clean: $(docsys lint --root docs | grep -E '^(ERROR|WARN)' | head -5)"
git add -A && git commit -qm "cartd: adopted, three finished work files, destinations prepared"
git tag seed
printf 'gen-project: %s at %s (seed)\n' "$DIR" "$(git rev-parse --short HEAD)"
