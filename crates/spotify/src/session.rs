use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, Notify};
use url::Url;

use crate::callback::{CALLBACK_PATH, CALLBACK_PORT, CallbackListener, LOGIN_TIMEOUT, redirect_uri};
use crate::error::{Result, SpotifyError};
use crate::models::{Page, RawName, RawPlaylist, RawTrackEntry, RawUser, SpotifyPlaylist, SpotifyStatus, SpotifyTrack, SpotifyUser};
use crate::pkce;
use crate::store::RefreshTokenStore;
use crate::token::{self, AccessToken, Clock};

pub const LIKED_PLAYLIST_ID: &str = "liked";

const MAX_RATE_LIMIT_RETRIES: u32 = 3;
const MAX_RATE_LIMIT_WAIT_SECS: u64 = 30;
const MAX_PAGES: usize = 2000;
const BIND_ATTEMPTS: u32 = 10;
const BIND_RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Clone)]
pub struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub api: String,
}

impl Endpoints {
    pub fn spotify() -> Self {
        Self {
            authorize: "https://accounts.spotify.com/authorize".into(),
            token: "https://accounts.spotify.com/api/token".into(),
            api: "https://api.spotify.com/v1".into(),
        }
    }
}

struct Inner {
    client_id: Option<String>,
    access: Option<AccessToken>,
    user: Option<SpotifyUser>,
    last_error: Option<String>,
    restore_attempted: bool,
}

pub struct Session {
    http: reqwest::Client,
    endpoints: Endpoints,
    store: Arc<dyn RefreshTokenStore>,
    clock: Clock,
    callback_port: u16,
    login_timeout: Duration,
    inner: Mutex<Inner>,
    login_cancel: std::sync::Mutex<Option<Arc<Notify>>>,
}

fn network_error(e: reqwest::Error) -> SpotifyError {
    SpotifyError::Network(e.without_url().to_string())
}

fn api_error_message(body: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    value["error"]["message"]
        .as_str()
        .or_else(|| value["error_description"].as_str())
        .or_else(|| value["error"].as_str())
        .unwrap_or("request failed")
        .to_string()
}

fn same_origin(a: &str, b: &str) -> bool {
    match (Url::parse(a), Url::parse(b)) {
        (Ok(a), Ok(b)) => a.scheme() == b.scheme() && a.host_str() == b.host_str() && a.port_or_known_default() == b.port_or_known_default(),
        _ => false,
    }
}

fn valid_playlist_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl Session {
    pub fn new(client_id: Option<String>, store: Arc<dyn RefreshTokenStore>, endpoints: Endpoints, clock: Clock) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("deckforge/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(network_error)?;
        Ok(Self {
            http,
            endpoints,
            store,
            clock,
            callback_port: CALLBACK_PORT,
            login_timeout: LOGIN_TIMEOUT,
            inner: Mutex::new(Inner {
                client_id: client_id.filter(|c| !c.is_empty()),
                access: None,
                user: None,
                last_error: None,
                restore_attempted: false,
            }),
            login_cancel: std::sync::Mutex::new(None),
        })
    }

    pub fn with_callback(mut self, port: u16, timeout: Duration) -> Self {
        self.callback_port = port;
        self.login_timeout = timeout;
        self
    }

    async fn store_load(&self) -> Result<Option<String>> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.load()).await.map_err(|e| SpotifyError::Store(e.to_string()))?
    }

    async fn store_save(&self, token: String) -> Result<()> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.save(&token)).await.map_err(|e| SpotifyError::Store(e.to_string()))?
    }

    async fn store_delete(&self) -> Result<()> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.delete()).await.map_err(|e| SpotifyError::Store(e.to_string()))?
    }

    pub async fn set_client_id(&self, client_id: Option<String>) -> Result<()> {
        let client_id = client_id.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
        let changed = {
            let mut inner = self.inner.lock().await;
            let changed = inner.client_id != client_id;
            if changed {
                inner.client_id = client_id;
                inner.access = None;
                inner.user = None;
                inner.last_error = None;
                inner.restore_attempted = false;
            }
            changed
        };
        if changed {
            self.store_delete().await?;
        }
        Ok(())
    }

    pub async fn status(&self) -> SpotifyStatus {
        self.restore_stored_session().await;
        let inner = self.inner.lock().await;
        SpotifyStatus {
            client_id: inner.client_id.clone(),
            connected: inner.access.is_some() && inner.user.is_some(),
            user: inner.user.clone(),
            redirect_uri: redirect_uri(),
            last_error: inner.last_error.clone(),
        }
    }

    async fn record_error(&self, error: &SpotifyError) {
        self.inner.lock().await.last_error = Some(error.to_string());
    }

    async fn restore_stored_session(&self) {
        {
            let mut inner = self.inner.lock().await;
            if inner.restore_attempted || inner.client_id.is_none() {
                return;
            }
            inner.restore_attempted = true;
        }
        match self.store_load().await {
            Ok(Some(_)) => {}
            Ok(None) => return,
            Err(e) => return self.record_error(&e).await,
        }
        let restored = async {
            self.access_token().await?;
            self.me().await.map(|_| ())
        }
        .await;
        if let Err(e) = restored {
            self.record_error(&e).await;
        }
    }

    async fn refresh_locked(&self, inner: &mut Inner) -> Result<()> {
        let client_id = inner.client_id.clone().ok_or(SpotifyError::NoClientId)?;
        let refresh_token = self.store_load().await?.ok_or(SpotifyError::NotConnected)?;
        match token::refresh(&self.http, &self.endpoints.token, &client_id, &refresh_token).await {
            Ok(response) => {
                inner.access = Some(AccessToken::from_response(&response, (self.clock)()));
                inner.last_error = None;
                if let Some(rotated) = response.refresh_token.filter(|t| *t != refresh_token) {
                    self.store_save(rotated).await?;
                }
                Ok(())
            }
            Err(e @ SpotifyError::InvalidGrant(_)) => {
                inner.access = None;
                inner.user = None;
                inner.last_error = Some(e.to_string());
                let _ = self.store_delete().await;
                Err(e)
            }
            Err(e) => Err(e),
        }
    }

    async fn access_token(&self) -> Result<String> {
        let mut inner = self.inner.lock().await;
        if let Some(access) = inner.access.as_ref().filter(|a| a.is_fresh((self.clock)())) {
            return Ok(access.value.clone());
        }
        self.refresh_locked(&mut inner).await?;
        inner.access.as_ref().map(|a| a.value.clone()).ok_or(SpotifyError::NotConnected)
    }

    async fn refresh_after_rejection(&self, rejected: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        if inner.access.as_ref().is_some_and(|a| a.value != rejected) {
            return Ok(());
        }
        self.refresh_locked(&mut inner).await
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut refreshed = false;
        let mut rate_limit_retries = 0;
        loop {
            let access = self.access_token().await?;
            let response = self.http.get(url).bearer_auth(&access).send().await.map_err(network_error)?;
            let status = response.status().as_u16();
            if status == 401 && !refreshed {
                refreshed = true;
                self.refresh_after_rejection(&access).await?;
                continue;
            }
            if status == 429 {
                let wait = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(1);
                if rate_limit_retries >= MAX_RATE_LIMIT_RETRIES || wait > MAX_RATE_LIMIT_WAIT_SECS {
                    return Err(SpotifyError::RateLimited(wait));
                }
                rate_limit_retries += 1;
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            let body = response.text().await.map_err(network_error)?;
            if !(200..300).contains(&status) {
                return Err(SpotifyError::Api { status, message: api_error_message(&body) });
            }
            return serde_json::from_str(&body).map_err(|e| SpotifyError::Decode(e.to_string()));
        }
    }

    async fn paginate<T: DeserializeOwned>(&self, first_url: String) -> Result<Vec<T>> {
        let mut out = Vec::new();
        let mut next = Some(first_url);
        for _ in 0..MAX_PAGES {
            let Some(url) = next else { break };
            let page: Page<T> = self.get_json(&url).await?;
            out.extend(page.items.into_iter().flatten());
            next = page.next.filter(|n| same_origin(n, &self.endpoints.api));
        }
        Ok(out)
    }

    pub async fn me(&self) -> Result<SpotifyUser> {
        let raw: RawUser = self.get_json(&format!("{}/me", self.endpoints.api)).await?;
        let user = raw.into_user();
        self.inner.lock().await.user = Some(user.clone());
        Ok(user)
    }

    pub async fn playlists(&self) -> Result<Vec<SpotifyPlaylist>> {
        let raw: Vec<RawPlaylist> = self.paginate(format!("{}/me/playlists?limit=50", self.endpoints.api)).await?;
        Ok(raw.into_iter().map(RawPlaylist::into_playlist).collect())
    }

    pub async fn playlist_name(&self, playlist_id: &str) -> Result<String> {
        if playlist_id == LIKED_PLAYLIST_ID {
            return Ok("Spotify Liked Songs".into());
        }
        if !valid_playlist_id(playlist_id) {
            return Err(SpotifyError::Api { status: 400, message: "invalid playlist id".into() });
        }
        let raw: RawName = self.get_json(&format!("{}/playlists/{playlist_id}?fields=name", self.endpoints.api)).await?;
        raw.name.filter(|n| !n.is_empty()).ok_or_else(|| SpotifyError::Decode("playlist has no name".into()))
    }

    pub async fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<SpotifyTrack>> {
        let api = &self.endpoints.api;
        let entries: Vec<RawTrackEntry> = if playlist_id == LIKED_PLAYLIST_ID {
            self.paginate(format!("{api}/me/tracks?limit=50")).await?
        } else {
            if !valid_playlist_id(playlist_id) {
                return Err(SpotifyError::Api { status: 400, message: "invalid playlist id".into() });
            }
            match self.paginate(format!("{api}/playlists/{playlist_id}/items?limit=100")).await {
                Err(SpotifyError::Api { status: 404, .. }) => {
                    self.paginate(format!("{api}/playlists/{playlist_id}/tracks?limit=100")).await?
                }
                Err(SpotifyError::Api { status: 403, .. }) => {
                    return Err(SpotifyError::Api {
                        status: 403,
                        message: "Spotify only returns tracks for playlists you own or collaborate on".into(),
                    });
                }
                other => other?,
            }
        };
        Ok(entries.into_iter().filter_map(RawTrackEntry::into_track).collect())
    }

    pub async fn login(&self, open_browser: impl FnOnce(&str) -> std::result::Result<(), String>) -> Result<()> {
        let client_id = self.inner.lock().await.client_id.clone().ok_or(SpotifyError::NoClientId)?;
        let cancel = Arc::new(Notify::new());
        if let Some(previous) = self.login_cancel.lock().unwrap().replace(cancel.clone()) {
            previous.notify_one();
        }
        let outcome = self.run_login(&client_id, &cancel, open_browser).await;
        {
            let mut slot = self.login_cancel.lock().unwrap();
            if slot.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cancel)) {
                *slot = None;
            }
        }
        match &outcome {
            Ok(()) => self.inner.lock().await.last_error = None,
            Err(SpotifyError::Cancelled) => {}
            Err(e) => self.record_error(e).await,
        }
        outcome
    }

    pub fn cancel_login(&self) {
        if let Some(cancel) = self.login_cancel.lock().unwrap().as_ref() {
            cancel.notify_one();
        }
    }

    async fn bind_listener(&self) -> Result<CallbackListener> {
        let mut attempt = 1;
        loop {
            match CallbackListener::bind(self.callback_port).await {
                Err(SpotifyError::Listen { .. }) if attempt < BIND_ATTEMPTS => {
                    attempt += 1;
                    tokio::time::sleep(BIND_RETRY_DELAY).await;
                }
                other => return other,
            }
        }
    }

    async fn run_login(
        &self,
        client_id: &str,
        cancel: &Notify,
        open_browser: impl FnOnce(&str) -> std::result::Result<(), String>,
    ) -> Result<()> {
        let verifier = pkce::generate_verifier()?;
        let state = pkce::generate_state()?;
        let listener = self.bind_listener().await?;
        let redirect = format!("http://127.0.0.1:{}{CALLBACK_PATH}", listener.port());
        let url = pkce::authorize_url(&self.endpoints.authorize, client_id, &redirect, &pkce::challenge_for(&verifier), &state)?;
        open_browser(&url).map_err(SpotifyError::Browser)?;
        let code = listener.wait_for_code(&state, self.login_timeout, cancel).await?;
        let response = token::exchange_code(&self.http, &self.endpoints.token, client_id, &code, &redirect, &verifier).await?;
        let refresh_token = response
            .refresh_token
            .clone()
            .ok_or_else(|| SpotifyError::Auth("Spotify did not return a refresh token".into()))?;
        self.store_save(refresh_token).await?;
        {
            let mut inner = self.inner.lock().await;
            inner.access = Some(AccessToken::from_response(&response, (self.clock)()));
            inner.user = None;
            inner.restore_attempted = true;
        }
        self.me().await.map(|_| ())
    }

    pub async fn logout(&self) -> Result<()> {
        self.cancel_login();
        {
            let mut inner = self.inner.lock().await;
            inner.access = None;
            inner.user = None;
            inner.last_error = None;
            inner.restore_attempted = true;
        }
        self.store_delete().await
    }
}
