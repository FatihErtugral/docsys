---
id: refresh
type: reference
verification: verified
verified_by: maintainer
verified_rev: @REV@
verified_hash: "sha256:57a73fe51a6922830f1947826ad8d84748135796ef64a7f8ee08ab5e22965309"
sources: []
updated: 2026-09-02
verifies:
  - path: src/auth.rs
    symbol: refresh_token
---
# Refresh

This page states how a token is refreshed; read it before changing the TTL.

A token is refreshed at half its TTL.
