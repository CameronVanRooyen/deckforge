<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { initNative, native, registerSurface, setNativeZoom, type DeckIndex, type SurfaceId } from './native.svelte';

  interface Props {
    id: SurfaceId;
    /** CSS px per track second; only meaningful for the scrolling surfaces. */
    zoom?: number;
    class?: string;
    /** Rendered instead of the native hole when the native layer is unavailable. */
    fallback?: Snippet;
  }
  let { id, zoom, class: cls = '', fallback }: Props = $props();

  let el: HTMLDivElement;
  const deck = $derived((id.endsWith('-b') ? 1 : 0) as DeckIndex);

  onMount(() => {
    void initNative();
    return registerSurface(id, el);
  });

  $effect(() => {
    if (native.available && zoom !== undefined && id.startsWith('scroll')) setNativeZoom(zoom, deck);
  });
</script>

<div bind:this={el} class="native-surface {cls}" data-native-surface={id} data-native={native.ready ? (native.available ? 'on' : 'off') : 'pending'}>
  {#if native.ready && !native.available}{@render fallback?.()}{/if}
</div>

<style>
  .native-surface {
    position: relative;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    background: transparent;
    pointer-events: none;
  }
  .native-surface[data-native='off'] {
    pointer-events: auto;
  }
</style>
