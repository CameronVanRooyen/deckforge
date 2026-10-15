use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::{Result, SpotifyError};

const VERIFIER_BYTES: usize = 64;

pub const SCOPES: &str = "playlist-read-private playlist-read-collaborative user-library-read user-read-private user-read-email";

fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| SpotifyError::Auth(format!("random generator unavailable: {e}")))?;
    Ok(buf)
}

pub fn generate_verifier() -> Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes::<VERIFIER_BYTES>()?))
}

pub fn generate_state() -> Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes::<16>()?))
}

pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn authorize_url(authorize_endpoint: &str, client_id: &str, redirect_uri: &str, challenge: &str, state: &str) -> Result<String> {
    let mut url = Url::parse(authorize_endpoint).map_err(|e| SpotifyError::Auth(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", challenge)
        .append_pair("state", state)
        .append_pair("scope", SCOPES);
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(challenge_for(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn verifier_has_valid_charset_and_length() {
        let v = generate_verifier().unwrap();
        assert!((43..=128).contains(&v.len()), "length {}", v.len());
        assert!(v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(v, generate_verifier().unwrap());
    }

    #[test]
    fn authorize_url_encodes_every_parameter() {
        let url = authorize_url(
            "https://accounts.spotify.com/authorize",
            "client id",
            "http://127.0.0.1:43821/callback",
            "chal-lenge_1",
            "st&te",
        )
        .unwrap();
        let parsed = Url::parse(&url).unwrap();
        assert_eq!(parsed.host_str(), Some("accounts.spotify.com"));
        let get = |k: &str| parsed.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());
        assert_eq!(get("client_id").as_deref(), Some("client id"));
        assert_eq!(get("response_type").as_deref(), Some("code"));
        assert_eq!(get("redirect_uri").as_deref(), Some("http://127.0.0.1:43821/callback"));
        assert_eq!(get("code_challenge_method").as_deref(), Some("S256"));
        assert_eq!(get("code_challenge").as_deref(), Some("chal-lenge_1"));
        assert_eq!(get("state").as_deref(), Some("st&te"));
        assert_eq!(get("scope").as_deref(), Some(SCOPES));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A43821%2Fcallback"));
        assert!(url.contains("state=st%26te"));
    }
}
