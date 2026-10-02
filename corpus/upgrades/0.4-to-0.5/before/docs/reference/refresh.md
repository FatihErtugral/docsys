---
id: refresh
type: reference
verification: verified
verified_by: maintainer
verified_rev: @REV@
sources: []
updated: 2026-09-02
verifies:
  - path: src/auth.rs
    symbol: refresh_token
    hash: "sha256:a77f96244f07278130a4e6341c56cce3830d8fcf0992a13101b71591dbdda9b7"
---
# Refresh

This page states how a token is refreshed; read it before changing the TTL.

A token is refreshed at half its TTL.
