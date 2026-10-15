<script lang="ts">
  import { spotify } from '$lib/state/spotify.svelte';
  import { library } from '$lib/state/library.svelte';
  import { toast } from '$lib/state/toast.svelte';
  import { ui } from '$lib/state/ui.svelte';
  import { startTrackDrag } from '$lib/state/dnd.svelte';
  import { fmtBpm, fmtDuration } from '$lib/util/format';
  import { keyCompat } from '$lib/util/camelot';
  import KeyBadge from './KeyBadge.svelte';
  import ArtThumb from './ArtThumb.svelte';
  import AudioLines from '@lucide/svelte/icons/audio-lines';
  import Copy from '@lucide/svelte/icons/copy';
  import Check from '@lucide/svelte/icons/check';
  import Info from '@lucide/svelte/icons/info';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import LogOut from '@lucide/svelte/icons/log-out';
  import ListPlus from '@lucide/svelte/icons/list-plus';
  import Heart from '@lucide/svelte/icons/heart';
  import ListMusic from '@lucide/svelte/icons/list-music';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import type { SpotifyTrack } from '$lib/api';

  const ROW = $derived(ui.compact ? 30 : 34);
  const status = $derived(spotify.status);
  let clientDraft = $state('');
  let editingClient = $state(false);
  let copied = $state(false);
  let scrollTop = $state(0);
  let viewH = $state(260);

  $effect(() => {
    if (!spotify.statusLoaded) void spotify.refreshStatus();
  });
  $effect(() => {
    clientDraft = status?.clientId ?? '';
  });

  const showSetup = $derived(!spotify.configured || editingClient);
  const rows = $derived(spotify.tracks);
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 6));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewH) / ROW) + 6));
  const visible = $derived(rows.slice(first, last));
  const matchedCount = $derived(rows.filter((t) => t.localTrackId !== null).length);

  async function copyRedirect() {
    if (!status) return;
    try {
      await navigator.clipboard.writeText(status.redirectUri);
      copied = true;
      toast.ok('Redirect URI copied');
      setTimeout(() => (copied = false), 1800);
    } catch {
      toast.error('Could not copy. Select the address and copy it by hand.');
    }
  }

  async function saveClient() {
    await spotify.saveClientId(clientDraft);
    if (spotify.configured) editingClient = false;
  }

  function rowDown(e: PointerEvent, t: SpotifyTrack) {
    if (t.localTrackId === null || e.button !== 0) return;
    startTrackDrag(e, [t.localTrackId], t.name);
  }
  function dbl(e: MouseEvent, t: SpotifyTrack) {
    if (t.localTrackId === null) return;
    library.loadToDeck(library.pickDeck(e.altKey ? 1 : e.shiftKey ? 0 : undefined), t.localTrackId);
  }
  const initials = (name: string) => name.trim().slice(0, 1).toUpperCase() || '?';
</script>

<div class="sp" style="--row:{ROW}px">
  {#if !spotify.statusLoaded}
    <div class="center"><LoaderCircle size={16} class="spin" /> Checking Spotify…</div>
  {:else if showSetup}
    <div class="center">
      <div class="card wide">
        <div class="hdr">
          <div class="mark"><AudioLines size={18} strokeWidth={1.9} /></div>
          <div>
            <h3>Set up Spotify</h3>
            <p class="lead">Link your own Spotify app so deckforge can read your playlists.</p>
          </div>
        </div>
        <ol>
          <li>
            <span class="n">1</span>
            <div class="grow">
              <b>Add this redirect URI</b> to your app in the Spotify developer dashboard.
              <div class="uri">
                <code class="num">{status?.redirectUri ?? '—'}</code>
                <button class="btn" onclick={copyRedirect} aria-label="Copy redirect URI">
                  {#if copied}<Check size={13} strokeWidth={2.4} />Copied{:else}<Copy size={13} strokeWidth={2.2} />Copy{/if}
                </button>
              </div>
            </div>
          </li>
          <li>
            <span class="n">2</span>
            <div class="grow">
              <b>Paste the Client ID</b> from the app’s settings.
              <form
                class="cid"
                onsubmit={(e) => {
                  e.preventDefault();
                  void saveClient();
                }}
              >
                <input type="text" bind:value={clientDraft} placeholder="Client ID" spellcheck="false" autocomplete="off" aria-label="Spotify client ID" onkeydown={(e) => e.stopPropagation()} />
                <button class="btn btn-primary" type="submit" disabled={!clientDraft.trim() || spotify.savingClient}>
                  {#if spotify.savingClient}<LoaderCircle size={13} class="spin" />{/if}Save
                </button>
                {#if editingClient}<button class="btn btn-ghost" type="button" onclick={() => (editingClient = false)}>Cancel</button>{/if}
              </form>
            </div>
          </li>
        </ol>
        {#if spotify.error}<p class="err"><TriangleAlert size={13} />{spotify.error}</p>{/if}
      </div>
    </div>
  {:else if !spotify.connected}
    <div class="center">
      <div class="card narrow">
        <div class="mark"><AudioLines size={18} strokeWidth={1.9} /></div>
        <h3>Connect Spotify</h3>
        <p class="lead">Sign in with Spotify in your browser. deckforge only reads your playlists and saved tracks.</p>
        <button class="btn btn-primary big" onclick={() => spotify.connect()} disabled={spotify.connecting}>
          {#if spotify.connecting}<LoaderCircle size={14} class="spin" />Waiting for your browser…{:else}Connect Spotify{/if}
        </button>
        {#if spotify.error}<p class="err"><TriangleAlert size={13} />{spotify.error}</p>{/if}
        <button class="link" onclick={() => (editingClient = true)}>Change client ID</button>
      </div>
    </div>
  {:else}
    <div class="top">
      <div class="user" title={status?.user?.displayName}>
        <span class="av">{initials(status?.user?.displayName ?? '')}</span>
        <b>{status?.user?.displayName ?? 'Spotify'}</b>
        {#if status?.user?.product}<span class="plan">{status.user.product}</span>{/if}
      </div>
      <div class="grow"></div>
      <button class="btn btn-ghost" onclick={() => spotify.disconnect()}><LogOut size={13} strokeWidth={2.1} />Log out</button>
    </div>
    <div class="cols">
      <div class="pls">
        {#if spotify.playlistsLoading}<div class="mini"><LoaderCircle size={13} class="spin" />Loading…</div>{/if}
        {#each spotify.playlists as p (p.id)}
          <button class="pl" class:sel={spotify.selectedId === p.id} onclick={() => spotify.select(p.id)}>
            {#if p.id === 'liked'}<Heart size={14} strokeWidth={1.9} />{:else}<ListMusic size={14} strokeWidth={1.9} />{/if}
            <span class="name">{p.name}</span>
            {#if p.trackCount}<span class="cnt num">{p.trackCount}</span>{/if}
          </button>
        {/each}
      </div>
      <div class="trk">
        <div class="thead">
          <div class="ttl">
            <b>{spotify.selected?.name ?? 'Select a playlist'}</b>
            {#if rows.length}<span class="meta">{matchedCount} of {rows.length} in your library</span>{/if}
          </div>
          <div class="grow"></div>
          <button class="btn btn-primary" onclick={() => spotify.importSelected()} disabled={!spotify.selectedId || spotify.importing || !matchedCount}>
            {#if spotify.importing}<LoaderCircle size={13} class="spin" />Importing…{:else}<ListPlus size={14} strokeWidth={2.1} />Import as playlist{/if}
          </button>
        </div>
        <div class="colhead">
          <span></span><span></span><span>Title</span><span>Artist</span><span class="r">BPM</span><span>Key</span><span class="r">Time</span><span>Status</span>
        </div>
        <div class="body" bind:clientHeight={viewH} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
          {#if spotify.tracksLoading}
            <div class="mini pad"><LoaderCircle size={13} class="spin" />Loading tracks…</div>
          {:else if spotify.error && !rows.length}
            <div class="mini pad err"><TriangleAlert size={13} />{spotify.error}</div>
          {:else if !rows.length}
            <div class="mini pad">{spotify.selectedId ? 'This playlist has no tracks.' : 'Pick a playlist on the left.'}</div>
          {:else}
            <div class="spacer" style="height:{rows.length * ROW}px">
              {#each visible as t, i (t.id)}
                {@const idx = first + i}
                {@const loc = t.localTrackId !== null ? spotify.local.get(t.localTrackId) : null}
                {@const matched = t.localTrackId !== null}
                {@const compat = matched && library.masterKey ? (keyCompat(library.masterKey, loc?.key) ?? 'dim') : null}
                <div
                  class="row"
                  class:off={!matched}
                  style="top:{idx * ROW}px"
                  role="row"
                  tabindex="-1"
                  onpointerdown={(e) => rowDown(e, t)}
                  ondblclick={(e) => dbl(e, t)}
                  title={matched ? 'Double-click or drag onto a deck' : 'Not in your library, so it can’t be loaded'}
                >
                  <span class="idx num">{idx + 1}</span>
                  <span class="art"><ArtThumb id={loc?.hasArtwork ? loc.id : null} size={ui.compact ? 22 : 26} /></span>
                  <span class="t title" title={t.name}>{t.name}</span>
                  <span class="t dim" title={t.artists.join(', ')}>{t.artists.join(', ')}</span>
                  <span class="t num r">{loc ? fmtBpm(loc.bpm, 1) : '—'}</span>
                  <span class="t">{#if loc}<KeyBadge value={loc.key} name={loc.keyName} size="sm" {compat} />{:else}<span class="faint">—</span>{/if}</span>
                  <span class="t num r dim">{fmtDuration(t.durationMs / 1000)}</span>
                  <span class="t st">
                    {#if matched}<span class="ok"><Check size={12} strokeWidth={2.6} />In library</span>{:else}<span class="tag">Not in library</span>{/if}
                  </span>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <p class="note"><Info size={12} strokeWidth={2.1} />Spotify tracks can’t be played in the decks, because Spotify doesn’t allow its audio in DJ software. deckforge matches them to your local files instead.</p>
</div>

<style>
  .sp {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--ink);
  }
  .center {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 8px;
    color: var(--dim);
    overflow: auto;
    padding: 12px;
  }
  .center > :global(*) {
    margin: auto;
  }
  .card {
    width: min(520px, 100%);
    padding: 14px 16px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--line);
    background: var(--panel);
  }
  .card.wide {
    width: min(780px, 100%);
  }
  .hdr {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .hdr .mark {
    margin-bottom: 0;
  }
  .hdr .lead {
    margin: 2px 0 0;
  }
  .card.narrow {
    width: min(380px, 100%);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }
  .mark {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: var(--radius);
    background: var(--raised-2);
    border: 1px solid var(--line-2);
    color: var(--text);
    margin-bottom: 8px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
    letter-spacing: -0.01em;
    color: var(--fg);
  }
  .lead {
    margin: 2px 0 12px;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--dim);
  }
  .narrow .lead {
    margin-bottom: 8px;
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px;
  }
  li {
    display: flex;
    gap: 10px;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--dim);
  }
  li b {
    color: var(--text);
    font-weight: 580;
  }
  .n {
    flex: none;
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    font-size: 11px;
    font-weight: 650;
    color: var(--dim);
    border: 1px solid var(--line-2);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .uri {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  code {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    height: 28px;
    padding: 0 9px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: #09090b;
    color: var(--text);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
    -webkit-user-select: text;
  }
  .cid {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .cid input {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 9px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: #09090b;
    outline: none;
    font-size: 12px;
    font-family: var(--font-mono);
    transition:
      border-color var(--dur),
      box-shadow var(--dur);
  }
  .cid input:focus {
    border-color: var(--ring);
    box-shadow: 0 0 0 3px rgba(161, 161, 170, 0.15);
  }
  .big {
    height: 32px;
    padding: 0 14px;
  }
  .link {
    margin-top: 6px;
    padding: 0;
    border: 0;
    background: none;
    font-size: 11.5px;
    color: var(--faint);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .link:hover {
    color: var(--dim);
  }
  .err {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 10px 0 0;
    font-size: 12px;
    color: #fca5a5;
  }
  .note {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 6px 12px;
    border-top: 1px solid var(--line);
    background: var(--panel);
    font-size: 11.5px;
    color: var(--faint);
  }
  .top {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 8px 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .user {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    height: 26px;
    padding: 0 10px 0 3px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--raised);
    font-size: 12px;
  }
  .user b {
    font-weight: 580;
    color: var(--fg);
    white-space: nowrap;
  }
  .av {
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--raised-3);
    color: var(--text);
    font-size: 10.5px;
    font-weight: 650;
  }
  .plan {
    font-size: 10.5px;
    color: var(--faint);
    text-transform: capitalize;
  }
  .cols {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
  }
  .pls {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 6px;
    overflow-y: auto;
    border-right: 1px solid var(--line);
    background: var(--panel);
    min-height: 0;
  }
  .pl {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    flex: none;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--dim);
    font-size: 12.5px;
    text-align: left;
    transition:
      background var(--dur),
      color var(--dur);
  }
  .pl:hover {
    background: rgba(255, 255, 255, 0.05);
    color: var(--fg);
  }
  .pl.sel {
    background: var(--raised-3);
    color: var(--fg);
    font-weight: 560;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cnt {
    font-size: 11px;
    color: var(--faint);
  }
  .trk {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .thead {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 10px 0 12px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .ttl {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .ttl b {
    font-size: 13px;
    font-weight: 620;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    font-size: 11.5px;
    color: var(--faint);
    white-space: nowrap;
  }
  .colhead,
  .row {
    display: grid;
    grid-template-columns: 34px 34px minmax(160px, 2.2fr) minmax(110px, 1.3fr) 60px 62px 56px 112px;
    align-items: center;
  }
  .colhead {
    flex: none;
    height: 24px;
    padding-right: 12px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    font-size: 11px;
    font-weight: 560;
    color: var(--faint);
  }
  .colhead span {
    padding: 0 8px;
  }
  .r {
    text-align: right;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    position: relative;
  }
  .spacer {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--row);
    font-size: 12.5px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    transition: background var(--dur);
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .row.off {
    opacity: 0.5;
  }
  .row.off:hover {
    background: transparent;
  }
  .idx {
    text-align: right;
    padding-right: 6px;
    font-size: 11px;
    color: var(--faint);
  }
  .art {
    padding: 0 4px;
    display: flex;
    align-items: center;
  }
  .t {
    padding: 0 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title {
    font-weight: 560;
    color: var(--fg);
  }
  .dim {
    color: var(--dim);
  }
  .faint {
    color: var(--faint);
  }
  .ok {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    color: #6ee7a0;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    height: 18px;
    padding: 0 6px;
    border-radius: 4px;
    border: 1px solid var(--line-2);
    font-size: 10.5px;
    font-weight: 560;
    color: var(--dim);
  }
  .mini {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px;
    font-size: 12px;
    color: var(--faint);
  }
  .mini.pad {
    padding: 18px 14px;
  }
</style>
