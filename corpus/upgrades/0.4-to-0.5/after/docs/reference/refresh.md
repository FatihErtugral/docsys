---
id: refresh
type: reference
sources: []
pins:
  - path: src/auth.rs
    symbol: refresh_token
---
# Refresh

This page states how a token is refreshed; read it before changing the TTL.

A token is refreshed at half its TTL.
