use std::sync::Mutex;

use crate::error::{Result, SpotifyError};

pub trait RefreshTokenStore: Send + Sync {
    fn load(&self) -> Result<Option<String>>;
    fn save(&self, token: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

pub struct KeyringStore {
    service: String,
    user: String,
}

impl KeyringStore {
    pub fn new(service: &str, user: &str) -> Self {
        Self { service: service.into(), user: user.into() }
    }

    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.user).map_err(|e| SpotifyError::Store(e.to_string()))
    }
}

impl RefreshTokenStore for KeyringStore {
    fn load(&self) -> Result<Option<String>> {
        match self.entry()?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SpotifyError::Store(e.to_string())),
        }
    }

    fn save(&self, token: &str) -> Result<()> {
        self.entry()?.set_password(token).map_err(|e| SpotifyError::Store(e.to_string()))
    }

    fn delete(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SpotifyError::Store(e.to_string())),
        }
    }
}

#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl MemoryStore {
    pub fn with_token(token: &str) -> Self {
        Self(Mutex::new(Some(token.to_string())))
    }
}

impl RefreshTokenStore for MemoryStore {
    fn load(&self) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, token: &str) -> Result<()> {
        *self.0.lock().unwrap() = Some(token.to_string());
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
