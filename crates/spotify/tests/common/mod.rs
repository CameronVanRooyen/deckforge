#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use deckforge_spotify::{Clock, Endpoints, MemoryStore, RefreshTokenStore, Session};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl Request {
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }

    pub fn bearer(&self) -> Option<&str> {
        self.headers.get("authorization").and_then(|v| v.strip_prefix("Bearer "))
    }

    pub fn form(&self) -> HashMap<String, String> {
        url::form_urlencoded::parse(self.body.as_bytes()).into_owned().collect()
    }
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Response {
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self { status, headers: vec![("Content-Type".into(), "application/json".into())], body: body.into() }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

pub struct MockServer {
    pub addr: SocketAddr,
    pub requests: Arc<Mutex<Vec<Request>>>,
}

impl MockServer {
    pub async fn start(handler: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(handler);
        let log = requests.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else { return };
                let handler = handler.clone();
                let log = log.clone();
                tokio::spawn(async move {
                    let Some(request) = read_request(&mut stream).await else { return };
                    let response = handler(&request);
                    log.lock().unwrap().push(request);
                    let mut out = format!("HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n", response.status, response.body.len());
                    for (name, value) in &response.headers {
                        out.push_str(&format!("{name}: {value}\r\n"));
                    }
                    out.push_str("\r\n");
                    out.push_str(&response.body);
                    let _ = stream.write_all(out.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self { addr, requests }
    }

    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn endpoints(&self) -> Endpoints {
        Endpoints { authorize: format!("{}/authorize", self.base()), token: format!("{}/api/token", self.base()), api: format!("{}/v1", self.base()) }
    }

    pub fn count(&self, path: &str) -> usize {
        self.requests.lock().unwrap().iter().filter(|r| r.path() == path).count()
    }

    pub fn all(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<Request> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.lines();
    let mut first = lines.next()?.split_whitespace();
    let (method, target) = (first.next()?.to_string(), first.next()?.to_string());
    let headers: HashMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let length: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    while buf.len() < header_end + length {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[header_end..]).into_owned();
    Some(Request { method, target, headers, body })
}

pub struct TestClock(pub Arc<AtomicU64>);

impl TestClock {
    pub fn new(start: u64) -> Self {
        Self(Arc::new(AtomicU64::new(start)))
    }

    pub fn clock(&self) -> Clock {
        let t = self.0.clone();
        Arc::new(move || t.load(Ordering::SeqCst))
    }

    pub fn advance(&self, secs: u64) {
        self.0.fetch_add(secs, Ordering::SeqCst);
    }
}

pub fn session(server: &MockServer, store: Arc<dyn RefreshTokenStore>, clock: &TestClock) -> Session {
    Session::new(Some("client-abc".into()), store, server.endpoints(), clock.clock()).unwrap()
}

pub fn store_with(token: &str) -> Arc<MemoryStore> {
    Arc::new(MemoryStore::with_token(token))
}

pub fn token_json(access: &str, refresh: Option<&str>) -> String {
    match refresh {
        Some(r) => format!(r#"{{"access_token":"{access}","token_type":"Bearer","expires_in":3600,"refresh_token":"{r}","scope":"x"}}"#),
        None => format!(r#"{{"access_token":"{access}","token_type":"Bearer","expires_in":3600,"scope":"x"}}"#),
    }
}

pub const ME_JSON: &str = r#"{"id":"gerhard","display_name":"Gerhard","images":[{"url":"https://i.scdn.co/me","width":300}]}"#;
