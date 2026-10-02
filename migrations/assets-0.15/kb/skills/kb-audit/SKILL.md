---
name: kb-audit
description: Independently verify knowledge-base pages against their sources — "audit my wiki", "verify these pages", "check the unverified pages". Records the audit or demotes the page.
---

# kb-audit — the independent eye

R-025: the session that produced a page never verifies it on its own
judgment. If you authored a page in this session, say so and stop —
verification needs another session, or the maintainer in this one: a declared
maintainer who reads the page and says it is right verifies it with
`docsys verify <page>` (their identity, their word — D-096).

For each `verification: unverified` page (or the ones named):

1. Read the page and every file in its `sources:`.
2. Judge faithfulness: is every claim supported? Any contradiction? A missing
   or empty source is a failure, not a pass.
3. **Faithful** → `docsys verify <page> --by "<who or which session>"` sets
   `verification: verified` and records the audit (R-028): `verified_by:` and
   `verified_rev:` (the base's current revision; the page must be committed
   as it is). Without that record the claim is unauditable.
4. **Not faithful** → leave/return it to `unverified` and append one line to
   `wiki/open-questions.md` naming the specific discrepancy —
   `- [ ] YYYY-MM-DD …` (R-108), in the base's language. Never edit the
   page's claims to make them pass — that is authoring, and it would need
   another audit.
5. Gate: `docsys lint --root <base>`.

Report page by page: verified, or demoted with the reason.
