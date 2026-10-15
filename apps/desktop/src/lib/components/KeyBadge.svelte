<script lang="ts">
  import { keyHue, parseCamelot, type KeyCompat } from '$lib/util/camelot';

  interface Props {
    value: string | null;
    name?: string | null;
    semitones?: number;
    compat?: KeyCompat | 'dim' | null;
    size?: 'sm' | 'md' | 'lg';
    original?: string | null;
  }
  let { value, name = null, semitones = 0, compat = null, size = 'md', original = null }: Props = $props();

  const hue = $derived(keyHue(value));
  const major = $derived(parseCamelot(value)?.letter === 'B');
  const title = $derived(
    !value
      ? 'No key'
      : semitones
        ? `${value}${name ? ` · ${name}` : ''} — shifted ${semitones > 0 ? '+' : '−'}${Math.abs(semitones)} st from ${original}`
        : `${value}${name ? ` · ${name}` : ''}`,
  );
</script>

{#if value}
  <span
    class="key {size}"
    class:hued={hue !== null}
    class:major
    class:shifted={semitones !== 0}
    data-compat={compat}
    style={hue === null ? '' : `--kh:${hue}`}
    {title}
  >
    {value}{#if semitones}<sup>{semitones > 0 ? '+' : '−'}{Math.abs(semitones)}</sup>{/if}
  </span>
{:else}
  <span class="key {size} none" {title}>—</span>
{/if}

<style>
  .key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    min-width: 30px;
    font-family: var(--font-mono);
    font-weight: 650;
    border-radius: 5px;
    border: 1px solid var(--line-2);
    color: var(--dim);
    background: var(--raised);
    white-space: nowrap;
    letter-spacing: -0.02em;
    transition: opacity var(--dur), box-shadow var(--dur);
  }
  .sm {
    height: 18px;
    padding: 0 5px;
    font-size: 11px;
  }
  .md {
    height: 22px;
    padding: 0 7px;
    font-size: 12px;
  }
  .lg {
    height: 28px;
    min-width: 40px;
    padding: 0 9px;
    font-size: 15px;
    border-radius: 6px;
  }
  .none {
    background: transparent;
    border-color: transparent;
    color: var(--faint);
  }
  .hued {
    color: hsl(var(--kh) 85% 72%);
    background: hsl(var(--kh) 45% 15%);
    border-color: hsl(var(--kh) 40% 28%);
  }
  .hued.major {
    background: hsl(var(--kh) 50% 23%);
    border-color: hsl(var(--kh) 45% 38%);
  }
  .shifted {
    border-style: dashed;
  }
  sup {
    font-size: 0.62em;
    font-weight: 700;
    line-height: 1;
    opacity: 0.85;
    transform: translateY(-0.28em);
  }
  [data-compat='dim'] {
    opacity: 0.4;
  }
  [data-compat='same'] {
    box-shadow: 0 0 0 1.5px hsl(var(--kh, 0) 85% 72% / 0.9);
  }
  [data-compat='adjacent'] {
    box-shadow: 0 0 0 1px hsl(var(--kh, 0) 85% 72% / 0.55);
  }
  [data-compat='relative'] {
    box-shadow: 0 0 0 1px hsl(var(--kh, 0) 85% 72% / 0.3);
  }
</style>
