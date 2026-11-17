<script lang="ts">
  // A labelled on/off switch: the whole row is the control (44 px on touch), the label is its name, the state is `aria-checked`. The knob moves on the
  // spring with transform only.
  let { label, checked = $bindable(false), hint, onchange }: { label: string; checked?: boolean; hint?: string; onchange?: (v: boolean) => void } = $props();
</script>

<button type="button" class="sw" role="switch" aria-checked={checked} onclick={() => { checked = !checked; onchange?.(checked); }}>
  <span class="tx"><span>{label}</span>{#if hint}<small>{hint}</small>{/if}</span>
  <i class="track"><b></b></i>
</button>

<style>
  .sw { display: flex; align-items: center; justify-content: space-between; gap: var(--s-5); width: 100%; min-height: var(--pill-h); text-align: start; font-size: var(--text-ui); }
  .tx { display: grid; min-width: 0; }
  small { font-size: var(--text-label); color: var(--ink-3); }
  .track { position: relative; flex: none; width: calc(var(--s-9) + var(--s-4)); height: calc(var(--s-8) + var(--s-1) * 2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.18); transition: background-color var(--dur-fast) var(--ease-out); }
  .track b { position: absolute; top: var(--s-1); left: var(--s-1); width: var(--s-8); height: var(--s-8); border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgb(0 0 0 / 0.3); transition: translate var(--dur-base) var(--spring); }
  .sw[aria-checked="true"] .track { background: var(--accent-base); }
  .sw[aria-checked="true"] .track b { translate: calc(var(--s-9) + var(--s-4) - var(--s-8) - var(--s-1) * 2) 0; }
</style>
