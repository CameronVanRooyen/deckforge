mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::*;
use deckforge_spotify::{RefreshTokenStore, SpotifyError, challenge_for};

fn track_json(id: &str, name: &str) -> String {
    format!(
        r#"{{"id":"{id}","type":"track","name":"{name}","artists":[{{"name":"Artist"}}],"album":{{"name":"Album","images":[{{"url":"https://i.scdn.co/{id}","width":300}}]}},"duration_ms":200000,"external_ids":{{"isrc":"ISRC{id}"}}}}"#
    )
}

#[tokio::test]
async fn follows_playlist_pagination_to_the_end() {
    let server = MockServer::start(|req| match (req.path(), req.target.contains("offset=2")) {
        ("/api/token", _) => Response::json(200, token_json("tok", None)),
        ("/v1/me/playlists", false) => Response::json(
            200,
            r#"{"items":[{"id":"a","name":"A","owner":{"display_name":"Me"},"tracks":{"total":1}},null],"next":"@@/v1/me/playlists?offset=2&limit=50"}"#
                .replace("@@", &format!("http://{}", req.headers["host"])),
        ),
        ("/v1/me/playlists", true) => Response::json(200, r#"{"items":[{"id":"b","name":"B","owner":{"id":"x"},"items":{"total":9}}],"next":null}"#),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    let playlists = session.playlists().await.unwrap();
    assert_eq!(playlists.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(playlists[1].track_count, 9);
    assert_eq!(server.count("/v1/me/playlists"), 2);
    assert!(server.all().iter().filter(|r| r.path().starts_with("/v1")).all(|r| r.bearer() == Some("tok")));
}

#[tokio::test]
async fn does_not_follow_next_links_to_another_origin() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/me/playlists" => Response::json(200, r#"{"items":[],"next":"https://evil.example/steal"}"#),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    assert!(session.playlists().await.unwrap().is_empty());
    assert_eq!(server.count("/v1/me/playlists"), 1);
}

#[tokio::test]
async fn playlist_tracks_skip_null_local_and_episode_items_across_pages() {
    let server = MockServer::start(|req| match (req.path(), req.target.contains("offset=2")) {
        ("/api/token", _) => Response::json(200, token_json("tok", None)),
        ("/v1/playlists/PL1/items", false) => Response::json(
            200,
            format!(
                r#"{{"items":[{{"is_local":false,"item":{}}},{{"is_local":false,"item":null}},{{"is_local":true,"item":{{"id":null,"type":"track","name":"local","is_local":true}}}}],"next":"http://{}/v1/playlists/PL1/items?offset=2"}}"#,
                track_json("t1", "One"),
                req.headers["host"]
            ),
        ),
        ("/v1/playlists/PL1/items", true) => Response::json(
            200,
            format!(r#"{{"items":[{{"track":{{"id":"e1","type":"episode","name":"ep"}}}},{{"track":{}}}],"next":null}}"#, track_json("t2", "Two")),
        ),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    let tracks = session.playlist_tracks("PL1").await.unwrap();
    assert_eq!(tracks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["One", "Two"]);
    assert_eq!(tracks[0].isrc.as_deref(), Some("ISRCt1"));
    assert_eq!(tracks[0].duration_ms, 200_000);
}

#[tokio::test]
async fn liked_tracks_use_the_saved_tracks_endpoint() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/me/tracks" => Response::json(200, format!(r#"{{"items":[{{"added_at":"x","track":{}}}],"next":null}}"#, track_json("l1", "Liked"))),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    let tracks = session.playlist_tracks("liked").await.unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].name, "Liked");
}

#[tokio::test]
async fn falls_back_to_legacy_tracks_path_and_explains_403() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/playlists/OLD/items" => Response::json(404, r#"{"error":{"status":404,"message":"Not found"}}"#),
        "/v1/playlists/OLD/tracks" => Response::json(200, format!(r#"{{"items":[{{"track":{}}}],"next":null}}"#, track_json("o1", "Old"))),
        "/v1/playlists/FORBIDDEN/items" => Response::json(403, r#"{"error":{"status":403,"message":"Forbidden"}}"#),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    assert_eq!(session.playlist_tracks("OLD").await.unwrap()[0].name, "Old");
    let err = session.playlist_tracks("FORBIDDEN").await.unwrap_err().to_string();
    assert!(err.contains("own or collaborate"), "{err}");
    assert!(session.playlist_tracks("../etc").await.is_err());
}

#[tokio::test]
async fn retries_429_using_retry_after_then_succeeds() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let server = MockServer::start(move |req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/me" if counter.fetch_add(1, Ordering::SeqCst) == 0 => Response::json(429, "{}").with_header("Retry-After", "0"),
        "/v1/me" => Response::json(200, ME_JSON),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    let user = session.me().await.unwrap();
    assert_eq!(user.display_name, "Gerhard");
    assert_eq!(server.count("/v1/me"), 2);
}

#[tokio::test]
async fn gives_up_after_bounded_429_retries_and_on_huge_retry_after() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/me" => Response::json(429, "{}").with_header("Retry-After", "0"),
        "/v1/me/playlists" => Response::json(429, "{}").with_header("Retry-After", "3600"),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    assert!(matches!(session.me().await, Err(SpotifyError::RateLimited(0))));
    assert_eq!(server.count("/v1/me"), 4);
    let err = session.playlists().await.unwrap_err();
    assert!(matches!(err, SpotifyError::RateLimited(3600)));
    assert_eq!(server.count("/v1/me/playlists"), 1);
    assert!(err.to_string().contains("3600"));
}

#[tokio::test]
async fn refreshes_once_on_401_and_rotates_the_stored_refresh_token() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => {
            let form = req.form();
            assert_eq!(form["grant_type"], "refresh_token");
            assert_eq!(form["client_id"], "client-abc");
            match form["refresh_token"].as_str() {
                "r1" => Response::json(200, token_json("stale", Some("r2"))),
                "r2" => Response::json(200, token_json("fresh", None)),
                other => panic!("unexpected refresh token {other}"),
            }
        }
        "/v1/me" if req.bearer() == Some("fresh") => Response::json(200, ME_JSON),
        "/v1/me" => Response::json(401, r#"{"error":{"status":401,"message":"The access token expired"}}"#),
        _ => Response::json(404, "{}"),
    })
    .await;
    let store = store_with("r1");
    let session = session(&server, store.clone(), &TestClock::new(0));
    assert_eq!(session.me().await.unwrap().id, "gerhard");
    assert_eq!(server.count("/api/token"), 2);
    assert_eq!(store.load().unwrap().as_deref(), Some("r2"));
}

#[tokio::test]
async fn proactively_refreshes_when_the_injected_clock_passes_expiry() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/me" => Response::json(200, ME_JSON),
        _ => Response::json(404, "{}"),
    })
    .await;
    let clock = TestClock::new(1_000);
    let session = session(&server, store_with("r1"), &clock);
    session.me().await.unwrap();
    session.me().await.unwrap();
    assert_eq!(server.count("/api/token"), 1);
    clock.advance(3_600);
    session.me().await.unwrap();
    assert_eq!(server.count("/api/token"), 2);
}

#[tokio::test]
async fn status_restores_a_stored_session_lazily_once() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/me" => Response::json(200, ME_JSON),
        _ => Response::json(404, "{}"),
    })
    .await;
    let session = session(&server, store_with("r1"), &TestClock::new(0));
    let status = session.status().await;
    assert!(status.connected);
    assert_eq!(status.user.unwrap().display_name, "Gerhard");
    assert_eq!(status.client_id.as_deref(), Some("client-abc"));
    assert_eq!(status.redirect_uri, "http://127.0.0.1:43821/callback");
    session.status().await;
    assert_eq!(server.count("/api/token"), 1);
}

#[tokio::test]
async fn revoked_refresh_token_is_cleared_and_reported() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(400, r#"{"error":"invalid_grant","error_description":"Refresh token revoked"}"#),
        _ => Response::json(404, "{}"),
    })
    .await;
    let store = store_with("r1");
    let session = session(&server, store.clone(), &TestClock::new(0));
    let status = session.status().await;
    assert!(!status.connected);
    assert!(status.last_error.unwrap().contains("revoked"));
    assert_eq!(store.load().unwrap(), None);
}

#[tokio::test]
async fn status_without_stored_token_or_client_id_is_disconnected() {
    let server = MockServer::start(|_| Response::json(404, "{}")).await;
    let empty = Arc::new(deckforge_spotify::MemoryStore::default());
    let status = session(&server, empty.clone(), &TestClock::new(0)).status().await;
    assert!(!status.connected && status.last_error.is_none());
    let no_client = deckforge_spotify::Session::new(None, store_with("r1"), server.endpoints(), TestClock::new(0).clock()).unwrap();
    assert!(!no_client.status().await.connected);
    assert!(matches!(no_client.login(|_| Ok(())).await, Err(SpotifyError::NoClientId)));
    assert!(server.all().is_empty());
}

#[tokio::test]
async fn changing_the_client_id_drops_the_old_session() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => Response::json(200, token_json("tok", None)),
        "/v1/me" => Response::json(200, ME_JSON),
        _ => Response::json(404, "{}"),
    })
    .await;
    let store = store_with("r1");
    let session = session(&server, store.clone(), &TestClock::new(0));
    assert!(session.status().await.connected);
    session.set_client_id(Some("  client-abc  ".into())).await.unwrap();
    assert!(session.status().await.connected);
    session.set_client_id(Some("other".into())).await.unwrap();
    let status = session.status().await;
    assert!(!status.connected);
    assert_eq!(status.client_id.as_deref(), Some("other"));
    assert_eq!(store.load().unwrap(), None);
    session.set_client_id(Some("  ".into())).await.unwrap();
    assert_eq!(session.status().await.client_id, None);
}

#[tokio::test]
async fn full_login_flow_exchanges_the_code_with_pkce_and_logout_clears_it() {
    let server = MockServer::start(|req| match req.path() {
        "/api/token" => {
            let form = req.form();
            assert_eq!(form["grant_type"], "authorization_code");
            assert_eq!(form["code"], "the-code");
            assert_eq!(form["client_id"], "client-abc");
            assert!(form["redirect_uri"].ends_with("/callback"));
            assert!((43..=128).contains(&form["code_verifier"].len()));
            Response::json(200, token_json("tok", Some("rt-new")))
        }
        "/v1/me" => Response::json(200, ME_JSON),
        _ => Response::json(404, "{}"),
    })
    .await;
    let store = Arc::new(deckforge_spotify::MemoryStore::default());
    let session = deckforge_spotify::Session::new(Some("client-abc".into()), store.clone(), server.endpoints(), TestClock::new(0).clock())
        .unwrap()
        .with_callback(0, Duration::from_secs(10));

    let authorize_url = Arc::new(std::sync::Mutex::new(String::new()));
    let seen = authorize_url.clone();
    session
        .login(move |url| {
            *seen.lock().unwrap() = url.to_string();
            let parsed = url::Url::parse(url).unwrap();
            let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
            let redirect = url::Url::parse(&query["redirect_uri"]).unwrap();
            let target = format!("{}?code=the-code&state={}", redirect.path(), query["state"]);
            let port = redirect.port().unwrap();
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
                stream.write_all(format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes()).await.unwrap();
                let mut sink = String::new();
                let _ = stream.read_to_string(&mut sink).await;
            });
            Ok(())
        })
        .await
        .unwrap();

    let url = url::Url::parse(&authorize_url.lock().unwrap()).unwrap();
    assert_eq!(url.path(), "/authorize");
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["code_challenge_method"], "S256");
    let verifier = server.all().iter().find(|r| r.path() == "/api/token").unwrap().form()["code_verifier"].clone();
    assert_eq!(query["code_challenge"], challenge_for(&verifier));

    assert_eq!(store.load().unwrap().as_deref(), Some("rt-new"));
    let status = session.status().await;
    assert!(status.connected);
    assert_eq!(status.user.unwrap().id, "gerhard");

    session.logout().await.unwrap();
    assert_eq!(store.load().unwrap(), None);
    assert!(!session.status().await.connected);
}

#[tokio::test]
async fn login_times_out_and_reports_browser_failure() {
    let server = MockServer::start(|_| Response::json(404, "{}")).await;
    let session = deckforge_spotify::Session::new(Some("c".into()), Arc::new(deckforge_spotify::MemoryStore::default()), server.endpoints(), TestClock::new(0).clock())
        .unwrap()
        .with_callback(0, Duration::from_millis(100));
    assert!(matches!(session.login(|_| Ok(())).await, Err(SpotifyError::Timeout)));
    assert!(session.status().await.last_error.unwrap().contains("timed out"));
    assert!(matches!(session.login(|_| Err("no browser".into())).await, Err(SpotifyError::Browser(_))));
}
