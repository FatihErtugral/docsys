# Contributing to docsys

docsys is built **spec-first**: the specification is the product, the binary is
its reference implementation. Contributions follow that order.

## The one rule that shapes all others

Every behavior traces to a numbered rule in [SPEC.md](SPEC.md) or a registered
decision in [corpus/DECISIONS.md](corpus/DECISIONS.md). Code that enforces
something the spec does not say — or fails to enforce something it does — is a
bug, whichever direction it errs.

## Changing behavior

1. **Start with the rule, not the code.** A new check, a changed severity, a
   new command flag — first write or amend the rule in SPEC.md (or add a
   D-entry when the spec deliberately leaves the choice to implementations).
2. **Add a conformance case.** Every behavior change ships with a corpus case
   under `corpus/cases/` carrying the *exact* expected findings. The harness
   fails on extra findings as hard as on missing ones — that is what keeps the
   checker from drifting noisy.
3. **Severity is doctrine, not taste** (§2.2): a finding *blocks* only when
   the damage would be irreversible or silently wrong; everything else warns,
   and every warning names the file that must change.
4. **Withdrawn, never renumbered.** A removed rule keeps its number with a
   WITHDRAWN marker. Rule numbers are citations; citations must not rot.

## Hard constraints

- **Zero runtime dependencies** (D-001). PRs adding a crate to
  `[dependencies]` will be declined — auditability and supply-chain surface
  are features. The standard library is the toolbox.
- **`unsafe` is forbidden** (`#[forbid(unsafe_code)]`), and clippy runs with
  `unwrap_used`, `expect_used`, `indexing_slicing`, `panic` denied in
  production code (tests may allow them file-level).
- **Language-neutral.** The tool embeds no natural language: no built-in
  heading names, no example text that presumes English or any other language
  in normative places. Configuration carries the project's language.
- **The tool never authors prose.** Deterministic work (moving bytes,
  validating structure) belongs to the binary; judgment (classification,
  writing) belongs to an agent or a human. Do not blur that boundary.

## Practical checklist before a PR

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs these plus a dogfood job (`docsys adopt` on a fresh repository) on
Linux, macOS, and Windows — all must be green.

- Keep commit messages plain and descriptive of the change.
- Update `CHANGELOG.md` under the appropriate version section.
- If your change resolves an ambiguity the spec left open, register the
  choice in `corpus/DECISIONS.md` with its reason.

## Reporting issues

The most valuable bug report is a **minimal tree**: the smallest `docs/`
layout (plus `.docmeta.yml`) that produces the wrong finding — it is usually
one commit away from becoming a conformance case. Second best: the exact
command, the output you got, and the rule number you believe it violates.
