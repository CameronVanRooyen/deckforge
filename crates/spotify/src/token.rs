use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::error::{Result, SpotifyError};

pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

const REFRESH_SKEW_SECS: u64 = 60;

pub fn system_clock() -> Clock {
    Arc::new(|| SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Clone)]
pub struct AccessToken {
    pub value: String,
    pub expires_at: u64,
}

impl AccessToken {
    pub fn from_response(response: &TokenResponse, now: u64) -> Self {
        Self { value: response.access_token.clone(), expires_at: now.saturating_add(response.expires_in) }
    }

    pub fn is_fresh(&self, now: u64) -> bool {
        now.saturating_add(REFRESH_SKEW_SECS) < self.expires_at
    }
}

#[derive(Deserialize)]
struct TokenErrorBody {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

pub fn parse_token_response(status: u16, body: &str) -> Result<TokenResponse> {
    if (200..300).contains(&status) {
        return serde_json::from_str(body).map_err(|e| SpotifyError::Decode(format!("token response: {e}")));
    }
    let (code, description) = match serde_json::from_str::<TokenErrorBody>(body) {
        Ok(e) => (e.error, e.error_description.unwrap_or_default()),
        Err(_) => (format!("http {status}"), String::new()),
    };
    let detail = if description.is_empty() { code.clone() } else { format!("{code}: {description}") };
    if code == "invalid_grant" {
        Err(SpotifyError::InvalidGrant(detail))
    } else {
        Err(SpotifyError::Auth(detail))
    }
}

pub async fn request_token(http: &reqwest::Client, token_url: &str, form: &[(&str, &str)]) -> Result<TokenResponse> {
    let body = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(form).finish();
    let response = http
        .post(token_url)
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| SpotifyError::Network(e.without_url().to_string()))?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(|e| SpotifyError::Network(e.without_url().to_string()))?;
    parse_token_response(status, &body)
}

pub async fn exchange_code(
    http: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenResponse> {
    request_token(
        http,
        token_url,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("code_verifier", verifier),
        ],
    )
    .await
}

pub async fn refresh(http: &reqwest::Client, token_url: &str, client_id: &str, refresh_token: &str) -> Result<TokenResponse> {
    request_token(
        http,
        token_url,
        &[("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", client_id)],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUCCESS: &str = r#"{"access_token":"BQD-token","token_type":"Bearer","expires_in":3600,
        "refresh_token":"AQD-refresh","scope":"playlist-read-private user-library-read"}"#;

    #[test]
    fn parses_token_response_with_refresh_token() {
        let r = parse_token_response(200, SUCCESS).unwrap();
        assert_eq!(r.access_token, "BQD-token");
        assert_eq!(r.refresh_token.as_deref(), Some("AQD-refresh"));
        assert_eq!(r.expires_in, 3600);
    }

    #[test]
    fn refresh_response_may_omit_refresh_token() {
        let r = parse_token_response(200, r#"{"access_token":"a","token_type":"Bearer","expires_in":3600,"scope":""}"#).unwrap();
        assert!(r.refresh_token.is_none());
    }

    #[test]
    fn invalid_grant_is_distinguished() {
        let body = r#"{"error":"invalid_grant","error_description":"Refresh token revoked"}"#;
        assert!(matches!(parse_token_response(400, body), Err(SpotifyError::InvalidGrant(m)) if m.contains("revoked")));
        let other = r#"{"error":"invalid_client"}"#;
        assert!(matches!(parse_token_response(400, other), Err(SpotifyError::Auth(m)) if m == "invalid_client"));
        assert!(matches!(parse_token_response(502, "<html>"), Err(SpotifyError::Auth(m)) if m.contains("502")));
    }

    #[test]
    fn expiry_uses_injected_time_with_skew() {
        let token = AccessToken::from_response(&parse_token_response(200, SUCCESS).unwrap(), 1_000);
        assert_eq!(token.expires_at, 4_600);
        assert!(token.is_fresh(1_000));
        assert!(token.is_fresh(4_539));
        assert!(!token.is_fresh(4_540));
        assert!(!token.is_fresh(9_999));
    }
}
