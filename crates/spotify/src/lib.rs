//! Spotify OAuth (PKCE), Web API client and library matcher for deckforge. Metadata only; no audio.

mod callback;
mod error;
mod matcher;
mod models;
mod pkce;
mod session;
mod store;
mod token;

pub use callback::{CALLBACK_PATH, CALLBACK_PORT, CallbackError, CallbackListener, LOGIN_TIMEOUT, parse_callback_request, redirect_uri};
pub use error::{Result, SpotifyError};
pub use matcher::{LibraryIndex, LocalTrack, MATCH_THRESHOLD, TrackMatch};
pub use models::{SpotifyPlaylist, SpotifyStatus, SpotifyTrack, SpotifyUser};
pub use pkce::{SCOPES, authorize_url, challenge_for, generate_state, generate_verifier};
pub use session::{Endpoints, LIKED_PLAYLIST_ID, Session};
pub use store::{KeyringStore, MemoryStore, RefreshTokenStore};
pub use token::{AccessToken, Clock, TokenResponse, parse_token_response, system_clock};
