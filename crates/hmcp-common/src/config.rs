use std::env;

/// Access token TTL in seconds. Default 24 hours.
pub fn access_token_ttl() -> u64 {
    env::var("HMCP_TOKEN_TTL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(86400)
}
