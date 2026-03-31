use std::env;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DbBackendType {
    Sqlite,
    Postgres,
}

impl DbBackendType {
    pub fn from_env() -> Self {
        match env::var("HMCP_DB_BACKEND").as_deref() {
            Ok("postgres") | Ok("postgresql") => Self::Postgres,
            _ => Self::Sqlite,
        }
    }
}

/// Access token TTL in seconds. Default 24 hours.
pub fn access_token_ttl() -> u64 {
    env::var("HMCP_TOKEN_TTL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(86400)
}
