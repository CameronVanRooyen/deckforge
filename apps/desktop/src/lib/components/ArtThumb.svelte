<script lang="ts">
  import { thumbnail } from '$lib/util/thumbs';
  import Music from '@lucide/svelte/icons/music';

  let { id, size = 26 }: { id: number | null; size?: number } = $props();
  let src = $state<string | null>(null);

  $effect(() => {
    const track = id;
    src = null;
    if (track === null) return;
    let live = true;
    thumbnail(track).then((u) => live && (src = u));
    return () => {
      live = false;
    };
  });
</script>

<span class="art" style="width:{size}px;height:{size}px">
  {#if src}<img {src} alt="" draggable="false" />{:else}<Music size={Math.round(size * 0.5)} strokeWidth={1.6} />{/if}
</span>

<style>
  .art {
    display: grid;
    place-items: center;
    flex: none;
    border-radius: 4px;
    overflow: hidden;
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--faint);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
