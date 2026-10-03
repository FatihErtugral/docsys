---
id: expiry
type: reference
sources: []
pins:
  - path: src/auth.rs
    symbol: expire
---
# Expiry

This page states when a token expires; read it before changing the lifetime.

A token expires one TTL after it was issued, or when it is revoked.
