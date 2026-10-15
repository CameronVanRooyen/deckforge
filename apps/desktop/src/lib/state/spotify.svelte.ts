import type { SpotifyPlaylist, SpotifyStatus, SpotifyTrack, TrackSummary } from '$lib/api';
import * as be from '$lib/api/backend';
import { errorMessage } from '$lib/util/format';
import { library } from './library.svelte';
import { toast } from './toast.svelte';

export const LIKED_ID = 'liked';

class SpotifyStore {
  status = $state.raw<SpotifyStatus | null>(null);
  statusLoaded = $state(false);
  connecting = $state(false);
  savingClient = $state(false);
  playlists = $state.raw<SpotifyPlaylist[]>([]);
  playlistsLoading = $state(false);
  selectedId = $state<string | null>(null);
  tracks = $state.raw<SpotifyTrack[]>([]);
  tracksLoading = $state(false);
  local = $state.raw<Map<number, TrackSummary>>(new Map());
  importing = $state(false);
  error = $state<string | null>(null);

  private tracksFor: string | null = null;

  get configured() {
    return !!this.status?.clientId;
  }
  get connected() {
    return !!this.status?.connected;
  }
  get selected() {
    return this.playlists.find((p) => p.id === this.selectedId) ?? null;
  }

  async refreshStatus() {
    try {
      this.status = await be.backend.spotifyStatus();
      this.error = this.status.lastError;
    } catch (e) {
      this.error = errorMessage(e);
    } finally {
      this.statusLoaded = true;
    }
    if (this.connected && !this.playlists.length) await this.loadPlaylists();
  }

  async saveClientId(clientId: string) {
    this.savingClient = true;
    try {
      this.status = await be.backend.spotifySetClientId(clientId.trim());
      this.error = this.status.lastError;
    } catch (e) {
      this.error = errorMessage(e);
      toast.error(`Could not save client ID: ${this.error}`);
    } finally {
      this.savingClient = false;
    }
  }

  async connect() {
    this.connecting = true;
    this.error = null;
    try {
      this.status = await be.backend.spotifyLogin();
      this.error = this.status.lastError;
      if (this.connected) {
        toast.ok(`Connected as ${this.status.user?.displayName ?? 'Spotify user'}`);
        await this.loadPlaylists();
      }
    } catch (e) {
      this.error = errorMessage(e);
    } finally {
      this.connecting = false;
    }
  }

  async disconnect() {
    try {
      this.status = await be.backend.spotifyLogout();
    } catch (e) {
      toast.error(errorMessage(e));
    }
    this.playlists = [];
    this.tracks = [];
    this.selectedId = null;
    this.tracksFor = null;
  }

  async loadPlaylists() {
    this.playlistsLoading = true;
    try {
      const list = await be.backend.spotifyPlaylists();
      this.playlists = list.some((p) => p.id === LIKED_ID)
        ? list
        : [{ id: LIKED_ID, name: 'Liked Songs', owner: 'You', trackCount: 0, imageUrl: null }, ...list];
      this.error = null;
      if (!this.selectedId && this.playlists.length) await this.select(this.playlists[0].id);
    } catch (e) {
      this.error = errorMessage(e);
    } finally {
      this.playlistsLoading = false;
    }
  }

  async select(id: string) {
    this.selectedId = id;
    this.tracksLoading = true;
    this.tracksFor = id;
    try {
      const tracks = await be.backend.spotifyPlaylistTracks(id);
      if (this.tracksFor !== id) return;
      this.tracks = tracks;
      this.error = null;
      void this.loadLocal(tracks);
    } catch (e) {
      if (this.tracksFor === id) {
        this.tracks = [];
        this.error = errorMessage(e);
      }
    } finally {
      if (this.tracksFor === id) this.tracksLoading = false;
    }
  }

  private async loadLocal(tracks: SpotifyTrack[]) {
    const ids = [...new Set(tracks.flatMap((t) => (t.localTrackId !== null ? [t.localTrackId] : [])))].filter((id) => !this.local.has(id)).slice(0, 400);
    for (let i = 0; i < ids.length; i += 24) {
      const batch = await Promise.allSettled(ids.slice(i, i + 24).map((id) => be.backend.track(id)));
      const next = new Map(this.local);
      for (const r of batch) if (r.status === 'fulfilled') next.set(r.value.id, r.value);
      this.local = next;
    }
  }

  async importSelected() {
    const id = this.selectedId;
    if (!id) return;
    this.importing = true;
    try {
      const r = await be.backend.spotifyImportPlaylist(id);
      toast.ok(`Imported “${r.playlist.name}” — ${r.matched} matched, ${r.unmatched} not in library`);
      await library.refreshPlaylists();
    } catch (e) {
      toast.error(`Import failed: ${errorMessage(e)}`);
    } finally {
      this.importing = false;
    }
  }
}

export const spotify = new SpotifyStore();
