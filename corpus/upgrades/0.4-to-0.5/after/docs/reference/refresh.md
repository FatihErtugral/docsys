---
id: refresh
type: reference
verification: verified
verified_by: maintainer
verified_rev: @REV@
verified_blocks: [3ec22832f118, c045be1837d0, 7e8f707e2ccf]
sources: []
updated: 2026-09-02
verifies:
  - path: src/auth.rs
    symbol: refresh_token
---
# Refresh

This page states how a token is refreshed; read it before changing the TTL.

A token is refreshed at half its TTL.
