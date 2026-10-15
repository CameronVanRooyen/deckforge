<script lang="ts" generics="T extends string">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Check from '@lucide/svelte/icons/check';

  interface Props {
    value: T;
    options: { value: T; label: string }[];
    onchange: (v: T) => void;
    label: string;
    width?: number;
    accent?: string;
    align?: 'start' | 'end';
  }
  let { value, options, onchange, label, width = 120, accent = 'var(--fg)', align = 'start' }: Props = $props();

  let open = $state(false);
  let active = $state(0);
  let btn: HTMLButtonElement;
  let pos = $state({ x: 0, y: 0, w: 0 });
  const current = $derived(options.find((o) => o.value === value));

  function toggle() {
    if (open) return (open = false);
    const r = btn.getBoundingClientRect();
    const w = Math.max(r.width, 132);
    pos = { x: align === 'end' ? r.right - w : r.left, y: r.bottom + 4, w };
    active = Math.max(0, options.findIndex((o) => o.value === value));
    open = true;
  }
  function pick(v: T) {
    open = false;
    onchange(v);
    btn.focus();
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      if (!open) return toggle();
      active = (active + (e.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length;
    } else if (open && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      e.stopPropagation();
      pick(options[active].value);
    } else if (open && e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      open = false;
    }
  }

  $effect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      const t = e.target as HTMLElement;
      if (!t.closest('.menu') && !btn.contains(t)) open = false;
    };
    const close = () => (open = false);
    window.addEventListener('pointerdown', away, true);
    window.addEventListener('blur', close);
    return () => {
      window.removeEventListener('pointerdown', away, true);
      window.removeEventListener('blur', close);
    };
  });
</script>

<button
  bind:this={btn}
  class="sel"
  class:open
  style="width:{width}px;--acc:{accent}"
  onclick={toggle}
  onkeydown={key}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-label={label}
>
  <span class="val">{current?.label ?? '—'}</span>
  <ChevronDown size={13} strokeWidth={2.2} />
</button>

{#if open}
  <div class="menu" role="listbox" aria-label={label} style="left:{pos.x}px;top:{pos.y}px;min-width:{pos.w}px">
    {#each options as o, i (o.value)}
      <div
        class="opt"
        class:active={i === active}
        role="option"
        tabindex="-1"
        aria-selected={o.value === value}
        onpointermove={() => (active = i)}
        onpointerdown={(e) => {
          e.preventDefault();
          pick(o.value);
        }}
      >
        <span>{o.label}</span>
        {#if o.value === value}<Check size={13} strokeWidth={2.4} />{/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .sel {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    height: 28px;
    padding: 0 8px 0 10px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--text);
    font-size: 12px;
    font-weight: 540;
    transition: border-color var(--dur), background var(--dur);
  }
  .sel:hover,
  .sel.open {
    background: var(--raised-2);
    border-color: var(--line-2);
  }
  .sel :global(svg) {
    color: var(--faint);
    flex: none;
  }
  .val {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .menu {
    position: fixed;
    z-index: 130;
    padding: 4px;
    border-radius: var(--radius-lg);
    background: #141416;
    border: 1px solid var(--line-2);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.6);
    animation: pop-in 100ms var(--ease);
  }
  .opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    height: 28px;
    padding: 0 8px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--text);
  }
  .opt.active {
    background: rgba(255, 255, 255, 0.08);
  }
  .opt :global(svg) {
    color: var(--acc);
  }
</style>
