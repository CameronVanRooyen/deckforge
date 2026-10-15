<script lang="ts">
  import { library } from '$lib/state/library.svelte';
  import { dropTargets } from '$lib/state/dnd.svelte';
  import { ui } from '$lib/state/ui.svelte';
  import { spotify } from '$lib/state/spotify.svelte';
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte';
  import Library from '@lucide/svelte/icons/library';
  import ListMusic from '@lucide/svelte/icons/list-music';
  import Plus from '@lucide/svelte/icons/plus';
  import AudioLines from '@lucide/svelte/icons/audio-lines';
  import { tick } from 'svelte';

  let creating = $state(false);
  let renamingId = $state<number | null>(null);
  let draft = $state('');
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);

  const inLibrary = $derived(ui.browserView === 'library');

  function openLibrary(id: number | null) {
    ui.browserView = 'library';
    library.selectPlaylist(id);
  }
  function openSpotify() {
    ui.browserView = 'spotify';
    library.focus = 'sidebar';
    if (!spotify.statusLoaded) void spotify.refreshStatus();
  }

  async function startCreate() {
    creating = true;
    draft = '';
    await tick();
    inputEl?.focus();
  }
  async function startRename(id: number, name: string) {
    renamingId = id;
    draft = name;
    await tick();
    inputEl?.focus();
    inputEl?.select();
  }
  async function commit() {
    const name = draft.trim();
    if (creating) {
      creating = false;
      if (name) {
        const p = await library.createPlaylist(name);
        if (p) openLibrary(p.id);
      }
    } else if (renamingId !== null) {
      const id = renamingId;
      renamingId = null;
      if (name) await library.renamePlaylist(id, name);
    }
  }
  function cancel() {
    creating = false;
    renamingId = null;
  }
  function onkey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter') commit();
    else if (e.key === 'Escape') cancel();
  }
  function openMenu(e: MouseEvent, id: number, name: string) {
    e.preventDefault();
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: 'Rename', onselect: () => startRename(id, name) },
        { divider: true },
        { label: 'Delete playlist', danger: true, onselect: () => library.deletePlaylist(id) },
      ],
    };
  }
</script>

<nav class="side" class:focused={library.focus === 'sidebar'} aria-label="Library sections" onpointerdown={() => (library.focus = 'sidebar')}>
  <button class="item" class:sel={inLibrary && library.playlistId === null} onclick={() => openLibrary(null)}>
    <Library size={14} strokeWidth={1.9} /><span class="name">Collection</span>
  </button>
  <button class="item" class:sel={ui.browserView === 'spotify'} onclick={openSpotify}>
    <AudioLines size={14} strokeWidth={1.9} /><span class="name">Spotify</span>
    <i class="led" data-state={spotify.connected ? 'ok' : 'off'} title={spotify.connected ? 'Connected' : 'Not connected'}></i>
  </button>
  <div class="group">
    <span class="tiny">Playlists</span>
    <button class="add" onclick={startCreate} aria-label="New playlist" title="New playlist"><Plus size={14} strokeWidth={2.2} /></button>
  </div>
  <div class="list">
    {#each library.playlists as p (p.id)}
      {#if renamingId === p.id}
        <input class="edit" bind:this={inputEl} bind:value={draft} onkeydown={onkey} onblur={commit} maxlength="80" aria-label="Playlist name" />
      {:else}
        <button
          class="item"
          class:sel={inLibrary && library.playlistId === p.id}
          class:drop={dropTargets.playlist === p.id}
          data-drop-playlist={p.id}
          onclick={() => openLibrary(p.id)}
          ondblclick={() => startRename(p.id, p.name)}
          oncontextmenu={(e) => openMenu(e, p.id, p.name)}
        >
          <ListMusic size={14} strokeWidth={1.9} /><span class="name">{p.name}</span><span class="count num">{p.trackCount}</span>
        </button>
      {/if}
    {/each}
    {#if creating}
      <input class="edit" bind:this={inputEl} bind:value={draft} onkeydown={onkey} onblur={commit} placeholder="Playlist name" maxlength="80" aria-label="New playlist name" />
    {:else if library.playlists.length === 0}
      <p class="hint">Create a playlist, then drag tracks onto it.</p>
    {/if}
  </div>
</nav>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .side {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 8px 6px;
    min-height: 0;
    overflow-y: auto;
    border-right: 1px solid var(--line);
    background: var(--panel);
    position: relative;
  }
  .side.focused::after {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .group {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 8px 4px;
  }
  .add {
    width: 22px;
    height: 22px;
    border-radius: 5px;
    border: 0;
    background: transparent;
    color: var(--dim);
    display: grid;
    place-items: center;
    transition: background var(--dur), color var(--dur);
  }
  .add:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--fg);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--dim);
    font-size: 12.5px;
    text-align: left;
    flex: none;
    transition: background var(--dur), color var(--dur);
  }
  .item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: var(--fg);
  }
  .item.sel {
    background: var(--raised-3);
    color: var(--fg);
    font-weight: 560;
  }
  .item.drop {
    background: color-mix(in srgb, var(--ok) 18%, transparent);
    box-shadow: inset 0 0 0 1px var(--ok);
    color: var(--fg);
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 11px;
    color: var(--faint);
  }
  .edit {
    height: 28px;
    margin: 1px 0;
    padding: 0 8px;
    border-radius: var(--radius);
    border: 1px solid var(--ring);
    background: #09090b;
    outline: none;
    font-size: 12.5px;
  }
  .hint {
    margin: 6px 8px;
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--faint);
  }
</style>
