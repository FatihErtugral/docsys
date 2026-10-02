---
id: expiry
type: reference
verification: verified
verified_by: maintainer
verified_rev: @REV@
sources: []
updated: 2026-09-02
verifies:
  - path: src/auth.rs
    symbol: expire
    hash: "sha256:a2a9c371bbe0d7cbcc52cb32000a73b1d30ebde71670aef770f15e1e1b4720e1"
---
# Expiry

This page states when a token expires; read it before changing the lifetime.

A token expires one TTL after it was issued, or when it is revoked.
