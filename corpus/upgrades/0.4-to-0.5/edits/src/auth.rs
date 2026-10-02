//! Token handling.

/// doc: refresh
pub fn refresh_token(ttl: u64) -> u64 {
    ttl / 2
}

// The lifetime ends here; the rules are in doc: expiry
pub fn expire(issued: u64, ttl: u64) -> u64 {
    issued.saturating_add(ttl)
}
