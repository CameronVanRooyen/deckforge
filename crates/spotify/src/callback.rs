use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use url::Url;

use crate::error::{Result, SpotifyError};

pub const CALLBACK_PORT: u16 = 43821;
pub const CALLBACK_PATH: &str = "/callback";
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);

const MAX_REQUEST_BYTES: usize = 8192;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

pub fn redirect_uri() -> String {
    format!("http://127.0.0.1:{CALLBACK_PORT}{CALLBACK_PATH}")
}

#[derive(Debug, PartialEq, Eq)]
pub enum CallbackError {
    Malformed,
    NotCallback,
    StateMismatch,
    Denied(String),
    MissingCode,
}

pub fn parse_callback_request(request: &str, expected_state: &str) -> std::result::Result<String, CallbackError> {
    let line = request.lines().next().ok_or(CallbackError::Malformed)?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next(), parts.next());
    let (Some("GET"), Some(target)) = (method, target) else { return Err(CallbackError::Malformed) };
    if !target.starts_with('/') {
        return Err(CallbackError::Malformed);
    }
    let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| CallbackError::Malformed)?;
    if url.path() != CALLBACK_PATH {
        return Err(CallbackError::NotCallback);
    }
    let param = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    if param("state").as_deref() != Some(expected_state) {
        return Err(CallbackError::StateMismatch);
    }
    if let Some(error) = param("error") {
        return Err(CallbackError::Denied(error));
    }
    param("code").filter(|c| !c.is_empty()).ok_or(CallbackError::MissingCode)
}

pub struct CallbackListener {
    listener: TcpListener,
}

impl CallbackListener {
    pub async fn bind(port: u16) -> Result<Self> {
        let addr = format!("127.0.0.1:{port}");
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| SpotifyError::Listen { addr: addr.clone(), reason: e.to_string() })?;
        Ok(Self { listener })
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    pub async fn wait_for_code(self, expected_state: &str, timeout: Duration, cancel: &Notify) -> Result<String> {
        tokio::select! {
            result = self.accept_until_callback(expected_state) => result,
            _ = tokio::time::sleep(timeout) => Err(SpotifyError::Timeout),
            _ = cancel.notified() => Err(SpotifyError::Cancelled),
        }
    }

    async fn accept_until_callback(&self, expected_state: &str) -> Result<String> {
        loop {
            let (stream, _) = self.listener.accept().await.map_err(|e| SpotifyError::Auth(e.to_string()))?;
            if let Some(result) = handle_connection(stream, expected_state).await {
                return result;
            }
        }
    }
}

async fn read_request(stream: &mut TcpStream) -> String {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let read = async {
        while buf.len() < MAX_REQUEST_BYTES && !buf.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
    };
    let _ = tokio::time::timeout(READ_TIMEOUT, read).await;
    String::from_utf8_lossy(&buf).into_owned()
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn page(message: &str) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>deckforge</title>\
         <body style=\"font-family:system-ui;background:#111;color:#eee;display:grid;place-items:center;height:100vh;margin:0\">\
         <p>{message}</p></body>"
    )
}

async fn handle_connection(mut stream: TcpStream, expected_state: &str) -> Option<Result<String>> {
    let request = read_request(&mut stream).await;
    match parse_callback_request(&request, expected_state) {
        Ok(code) => {
            respond(&mut stream, "200 OK", &page("Spotify connected. You can close this tab and return to deckforge.")).await;
            Some(Ok(code))
        }
        Err(CallbackError::Malformed | CallbackError::NotCallback) => {
            respond(&mut stream, "404 Not Found", &page("Not found.")).await;
            None
        }
        Err(CallbackError::StateMismatch) => {
            respond(&mut stream, "400 Bad Request", &page("Login rejected: state mismatch. You can close this tab.")).await;
            Some(Err(SpotifyError::Auth("state mismatch; the login response did not match this request".into())))
        }
        Err(CallbackError::Denied(reason)) => {
            respond(&mut stream, "200 OK", &page("Spotify login was not completed. You can close this tab.")).await;
            Some(Err(SpotifyError::Auth(format!("Spotify returned: {reason}"))))
        }
        Err(CallbackError::MissingCode) => {
            respond(&mut stream, "400 Bad Request", &page("Login response had no code. You can close this tab.")).await;
            Some(Err(SpotifyError::Auth("callback did not include an authorization code".into())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = "abc123";

    #[test]
    fn parses_success() {
        let req = "GET /callback?code=AQD%2Bxyz&state=abc123 HTTP/1.1\r\nHost: 127.0.0.1:43821\r\n\r\n";
        assert_eq!(parse_callback_request(req, STATE), Ok("AQD+xyz".to_string()));
    }

    #[test]
    fn reports_access_denied() {
        let req = "GET /callback?error=access_denied&state=abc123 HTTP/1.1\r\n\r\n";
        assert_eq!(parse_callback_request(req, STATE), Err(CallbackError::Denied("access_denied".into())));
    }

    #[test]
    fn rejects_state_mismatch_and_missing_state() {
        let wrong = "GET /callback?code=x&state=evil HTTP/1.1\r\n\r\n";
        assert_eq!(parse_callback_request(wrong, STATE), Err(CallbackError::StateMismatch));
        let missing = "GET /callback?code=x HTTP/1.1\r\n\r\n";
        assert_eq!(parse_callback_request(missing, STATE), Err(CallbackError::StateMismatch));
    }

    #[test]
    fn rejects_garbage_and_other_paths() {
        assert_eq!(parse_callback_request("", STATE), Err(CallbackError::Malformed));
        assert_eq!(parse_callback_request("\x00\x01garbage", STATE), Err(CallbackError::Malformed));
        assert_eq!(parse_callback_request("POST /callback?code=x&state=abc123 HTTP/1.1", STATE), Err(CallbackError::Malformed));
        assert_eq!(parse_callback_request("GET /favicon.ico HTTP/1.1", STATE), Err(CallbackError::NotCallback));
        assert_eq!(parse_callback_request("GET /callback?state=abc123 HTTP/1.1", STATE), Err(CallbackError::MissingCode));
    }

    async fn http_get(port: u16, target: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let req = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn listener_delivers_code_from_a_real_http_request() {
        let listener = CallbackListener::bind(0).await.unwrap();
        let port = listener.port();
        let cancel = std::sync::Arc::new(Notify::new());
        let waiter = tokio::spawn({
            let cancel = cancel.clone();
            async move { listener.wait_for_code(STATE, Duration::from_secs(10), &cancel).await }
        });
        let stray = http_get(port, "/favicon.ico").await;
        assert!(stray.starts_with("HTTP/1.1 404"));
        let ok = http_get(port, "/callback?code=the-code&state=abc123").await;
        assert!(ok.starts_with("HTTP/1.1 200"));
        assert!(ok.contains("You can close this tab"));
        assert_eq!(waiter.await.unwrap().unwrap(), "the-code");
    }

    #[tokio::test]
    async fn listener_fails_on_state_mismatch() {
        let listener = CallbackListener::bind(0).await.unwrap();
        let port = listener.port();
        let cancel = Notify::new();
        let (result, response) = tokio::join!(
            listener.wait_for_code(STATE, Duration::from_secs(10), &cancel),
            http_get(port, "/callback?code=x&state=nope")
        );
        assert!(response.starts_with("HTTP/1.1 400"));
        assert!(matches!(result, Err(SpotifyError::Auth(_))));
    }

    #[tokio::test]
    async fn listener_times_out_and_cancels() {
        let listener = CallbackListener::bind(0).await.unwrap();
        let cancel = Notify::new();
        let result = listener.wait_for_code(STATE, Duration::from_millis(50), &cancel).await;
        assert!(matches!(result, Err(SpotifyError::Timeout)));

        let listener = CallbackListener::bind(0).await.unwrap();
        let cancel = Notify::new();
        cancel.notify_one();
        let result = listener.wait_for_code(STATE, Duration::from_secs(10), &cancel).await;
        assert!(matches!(result, Err(SpotifyError::Cancelled)));
    }
}
