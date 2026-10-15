use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyUser {
    pub id: String,
    pub display_name: String,
    pub product: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyStatus {
    pub client_id: Option<String>,
    pub connected: bool,
    pub user: Option<SpotifyUser>,
    pub redirect_uri: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyPlaylist {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub track_count: u64,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyTrack {
    pub id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: u64,
    pub isrc: Option<String>,
    pub image_url: Option<String>,
    pub local_track_id: Option<i64>,
    pub match_score: Option<f64>,
}

#[derive(Deserialize)]
pub(crate) struct Page<T> {
    #[serde(default = "Vec::new")]
    pub items: Vec<Option<T>>,
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct RawImage {
    url: String,
    #[serde(default)]
    width: Option<u32>,
}

const PREFERRED_IMAGE_WIDTH: u32 = 150;

fn pick_image(images: Option<Vec<RawImage>>) -> Option<String> {
    let images = images?;
    let smallest_adequate = images
        .iter()
        .filter(|i| i.width.is_some_and(|w| w >= PREFERRED_IMAGE_WIDTH))
        .min_by_key(|i| i.width);
    smallest_adequate.or_else(|| images.first()).map(|i| i.url.clone())
}

#[derive(Deserialize)]
pub(crate) struct RawUser {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    product: Option<String>,
    #[serde(default)]
    images: Option<Vec<RawImage>>,
}

impl RawUser {
    pub fn into_user(self) -> SpotifyUser {
        let display_name = self.display_name.filter(|n| !n.is_empty()).unwrap_or_else(|| self.id.clone());
        SpotifyUser { id: self.id, display_name, product: self.product, image_url: pick_image(self.images) }
    }
}

#[derive(Deserialize)]
struct RawOwner {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct RawCount {
    #[serde(default)]
    total: Option<u64>,
}

#[derive(Deserialize)]
pub(crate) struct RawPlaylist {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    owner: Option<RawOwner>,
    #[serde(default, alias = "tracks")]
    items: Option<RawCount>,
    #[serde(default)]
    images: Option<Vec<RawImage>>,
}

impl RawPlaylist {
    pub fn into_playlist(self) -> SpotifyPlaylist {
        let owner = self
            .owner
            .and_then(|o| o.display_name.filter(|n| !n.is_empty()).or(o.id))
            .unwrap_or_default();
        SpotifyPlaylist {
            id: self.id,
            name: self.name,
            owner,
            track_count: self.items.and_then(|c| c.total).unwrap_or(0),
            image_url: pick_image(self.images),
        }
    }
}

#[derive(Deserialize)]
struct RawArtist {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct RawAlbum {
    #[serde(default)]
    name: String,
    #[serde(default)]
    images: Option<Vec<RawImage>>,
}

#[derive(Deserialize)]
struct RawExternalIds {
    #[serde(default)]
    isrc: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct RawTrack {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    artists: Option<Vec<RawArtist>>,
    #[serde(default)]
    album: Option<RawAlbum>,
    #[serde(default)]
    duration_ms: u64,
    #[serde(default)]
    external_ids: Option<RawExternalIds>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    is_local: bool,
}

impl RawTrack {
    pub fn into_track(self) -> Option<SpotifyTrack> {
        if self.is_local || self.kind.as_deref().is_some_and(|k| k != "track") {
            return None;
        }
        let id = self.id.filter(|id| !id.is_empty())?;
        let (album, image_url) = match self.album {
            Some(a) => (a.name, pick_image(a.images)),
            None => (String::new(), None),
        };
        Some(SpotifyTrack {
            id,
            name: self.name,
            artists: self.artists.unwrap_or_default().into_iter().map(|a| a.name).filter(|n| !n.is_empty()).collect(),
            album,
            duration_ms: self.duration_ms,
            isrc: self.external_ids.and_then(|e| e.isrc).filter(|i| !i.is_empty()),
            image_url,
            local_track_id: None,
            match_score: None,
        })
    }
}

#[derive(Deserialize)]
pub(crate) struct RawTrackEntry {
    #[serde(default, alias = "item")]
    pub track: Option<RawTrack>,
    #[serde(default)]
    pub is_local: bool,
}

impl RawTrackEntry {
    pub fn into_track(self) -> Option<SpotifyTrack> {
        if self.is_local {
            return None;
        }
        self.track?.into_track()
    }
}

#[derive(Deserialize)]
pub(crate) struct RawName {
    #[serde(default)]
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYLISTS_PAGE: &str = r#"{
      "href": "https://api.spotify.com/v1/me/playlists?offset=0&limit=50",
      "limit": 50, "next": "https://api.spotify.com/v1/me/playlists?offset=50&limit=50", "offset": 0, "total": 3,
      "items": [
        {"id": "37i9dQZF1DXcBWIGoYBM5M", "name": "Peak Time Techno", "owner": {"id": "gerhard", "display_name": "Gerhard"},
         "tracks": {"href": "x", "total": 42},
         "images": [{"url": "https://i.scdn.co/image/big", "height": 640, "width": 640},
                    {"url": "https://i.scdn.co/image/mid", "height": 300, "width": 300},
                    {"url": "https://i.scdn.co/image/small", "height": 60, "width": 60}]},
        null,
        {"id": "p2", "name": "New API shape", "owner": {"id": "spotify"}, "items": {"total": 7}, "images": null}
      ]}"#;

    const TRACKS_PAGE: &str = r#"{
      "next": null, "total": 5,
      "items": [
        {"added_at": "2026-01-01T00:00:00Z", "is_local": false, "track": {
          "id": "4uLU6hMCjMI75M1A2tKUQC", "type": "track", "name": "Never Gonna Give You Up",
          "artists": [{"id": "a", "name": "Rick Astley"}, {"id": "b", "name": "Someone"}],
          "album": {"name": "Whenever You Need Somebody", "images": [{"url": "https://i.scdn.co/a640", "width": 640}, {"url": "https://i.scdn.co/a300", "width": 300}]},
          "duration_ms": 213573, "external_ids": {"isrc": "GBARL9300135"}}},
        {"added_at": "2026-01-01T00:00:00Z", "is_local": false, "track": null},
        {"added_at": "2026-01-01T00:00:00Z", "is_local": true, "track": {
          "id": null, "type": "track", "name": "My Local File", "artists": [{"name": "Me"}], "album": {"name": "", "images": []}, "duration_ms": 1000, "is_local": true}},
        {"added_at": "2026-01-01T00:00:00Z", "is_local": false, "track": {
          "id": "ep1", "type": "episode", "name": "Podcast Episode", "duration_ms": 1000}},
        {"added_at": "2026-01-01T00:00:00Z", "is_local": false, "item": {
          "id": "newshape", "type": "track", "name": "Renamed Field", "artists": [{"name": "A"}],
          "album": {"name": "B", "images": []}, "duration_ms": 180000, "external_ids": {}}}
      ]}"#;

    #[test]
    fn parses_playlist_page_including_null_entries_and_renamed_fields() {
        let page: Page<RawPlaylist> = serde_json::from_str(PLAYLISTS_PAGE).unwrap();
        assert!(page.next.as_deref().unwrap().contains("offset=50"));
        let playlists: Vec<_> = page.items.into_iter().flatten().map(RawPlaylist::into_playlist).collect();
        assert_eq!(playlists.len(), 2);
        assert_eq!(playlists[0].name, "Peak Time Techno");
        assert_eq!(playlists[0].owner, "Gerhard");
        assert_eq!(playlists[0].track_count, 42);
        assert_eq!(playlists[0].image_url.as_deref(), Some("https://i.scdn.co/image/mid"));
        assert_eq!(playlists[1].owner, "spotify");
        assert_eq!(playlists[1].track_count, 7);
        assert_eq!(playlists[1].image_url, None);
    }

    #[test]
    fn skips_null_local_and_episode_items() {
        let page: Page<RawTrackEntry> = serde_json::from_str(TRACKS_PAGE).unwrap();
        let tracks: Vec<_> = page.items.into_iter().flatten().filter_map(RawTrackEntry::into_track).collect();
        assert_eq!(tracks.len(), 2);
        let first = &tracks[0];
        assert_eq!(first.id, "4uLU6hMCjMI75M1A2tKUQC");
        assert_eq!(first.artists, ["Rick Astley", "Someone"]);
        assert_eq!(first.album, "Whenever You Need Somebody");
        assert_eq!(first.duration_ms, 213_573);
        assert_eq!(first.isrc.as_deref(), Some("GBARL9300135"));
        assert_eq!(first.image_url.as_deref(), Some("https://i.scdn.co/a300"));
        assert_eq!(tracks[1].id, "newshape");
        assert_eq!(tracks[1].isrc, None);
    }

    #[test]
    fn user_falls_back_to_id_without_display_name() {
        let user: RawUser = serde_json::from_str(r#"{"id":"abc","display_name":null,"images":[]}"#).unwrap();
        let user = user.into_user();
        assert_eq!(user.display_name, "abc");
        assert_eq!(user.product, None);
        assert_eq!(user.image_url, None);
    }

    #[test]
    fn serialises_camel_case() {
        let t = SpotifyTrack {
            id: "i".into(),
            name: "n".into(),
            artists: vec![],
            album: "a".into(),
            duration_ms: 1,
            isrc: None,
            image_url: None,
            local_track_id: Some(3),
            match_score: Some(0.9),
        };
        let v = serde_json::to_value(t).unwrap();
        assert_eq!(v["durationMs"], 1);
        assert_eq!(v["localTrackId"], 3);
        assert_eq!(v["matchScore"], 0.9);
    }
}
