#[derive(Debug, thiserror::Error)]
pub enum SpotifyError {
    #[error("Set a Spotify client ID first")]
    NoClientId,
    #[error("Not connected to Spotify")]
    NotConnected,
    #[error("Spotify login failed: {0}")]
    Auth(String),
    #[error("Spotify session expired or was revoked ({0}); connect again")]
    InvalidGrant(String),
    #[error("Spotify login timed out")]
    Timeout,
    #[error("Spotify login was cancelled")]
    Cancelled,
    #[error("Could not listen on {addr}: {reason}")]
    Listen { addr: String, reason: String },
    #[error("Could not open the browser: {0}")]
    Browser(String),
    #[error("Secure storage error: {0}")]
    Store(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Spotify rate limit hit; retry in {0} seconds")]
    RateLimited(u64),
    #[error("Spotify error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("Unexpected Spotify response: {0}")]
    Decode(String),
}

pub type Result<T> = std::result::Result<T, SpotifyError>;
