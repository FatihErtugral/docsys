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
---
# Expiry

This page states when a token expires; read it before changing the lifetime.

A token expires one TTL after it was issued, or when it is revoked.
