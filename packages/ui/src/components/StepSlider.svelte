<script lang="ts">
  // A slider with a few fixed positions: it snaps, it never rests between two, and it says each position in words. Pointer: a press or a drag moves the knob to the nearest step (the knob eases
  // between steps; there is no free travel). Keyboard: Left/Down and Right/Up move one step, Home and End the ends, PageUp and PageDown one step. The ends are labelled with the first and the last
  // step's name; `aria-valuetext` is the current step's name. `value` is the step's index; `onchange` gets the new index when it changes.
  let { label, steps, value, onchange, hint }: { label: string; steps: string[]; value: number; onchange: (index: number) => void; hint?: string } = $props();
  const last = $derived(steps.length - 1);
  const labelId = $props.id();
  let track = $state<HTMLElement>();
  const set = (i: number) => { const n = Math.min(last, Math.max(0, Math.round(i))); if (n !== value) onchange(n); };
  function at(e: PointerEvent): number {
    const r = track!.getBoundingClientRect();
    return ((e.clientX - r.left) / Math.max(1, r.width)) * last;
  }
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    track!.setPointerCapture(e.pointerId);
    set(at(e));
    (e.currentTarget as HTMLElement).focus({ preventScroll: true });
  }
  function move(e: PointerEvent) { if (track?.hasPointerCapture(e.pointerId)) set(at(e)); }
  function key(e: KeyboardEvent) {
    const k = e.key;
    const to = k === "ArrowLeft" || k === "ArrowDown" || k === "PageDown" ? value - 1 : k === "ArrowRight" || k === "ArrowUp" || k === "PageUp" ? value + 1 : k === "Home" ? 0 : k === "End" ? last : null;
    if (to === null) return;
    e.preventDefault();
    set(to);
  }
</script>

<div class="ss">
  <span class="name" id={labelId}>{label}</span>
  <output class="now">{steps[value]}</output>
  <div class="rail" role="slider" tabindex="0" aria-labelledby={labelId} aria-valuemin={0} aria-valuemax={last} aria-valuenow={value} aria-valuetext={steps[value]} aria-orientation="horizontal" bind:this={track} onpointerdown={down} onpointermove={move} onkeydown={key} style:--n={last} style:--i={value}>
    <i class="bar"></i>
    {#each steps as _, k}<i class="tick" class:on={k <= value} style:left="{(k / last) * 100}%"></i>{/each}
    <b class="knob"></b>
  </div>
  <span class="end l" aria-hidden="true">{steps[0]}</span>
  <span class="end r" aria-hidden="true">{steps[last]}</span>
  {#if hint}<small class="hint">{hint}</small>{/if}
</div>

<style>
  .ss { display: grid; grid-template-columns: 1fr 1fr; gap: var(--s-1) var(--s-4); align-items: center; }
  .name { font-weight: var(--strong-weight); }
  .now { justify-self: end; color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .rail { grid-column: 1 / -1; position: relative; height: var(--s-9); margin: 0 var(--s-4); touch-action: none; cursor: pointer; border-radius: var(--r-pill); }
  .bar { position: absolute; left: 0; right: 0; top: 50%; height: var(--s-2); translate: 0 -50%; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.18); }
  .tick { position: absolute; top: 50%; width: var(--s-2); height: var(--s-2); translate: -50% -50%; border-radius: 50%; background: rgb(var(--ink-rgb) / 0.4); transition: background-color var(--dur-fast) var(--ease-out); }
  .tick.on { background: var(--accent-base); }
  .knob { position: absolute; top: 50%; left: calc(var(--i) / var(--n) * 100%); width: var(--s-8); height: var(--s-8); translate: -50% -50%; border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgb(0 0 0 / 0.3), 0 0 0 1px rgb(var(--ink-rgb) / 0.08); transition: left var(--dur-base) var(--spring); }
  .rail:active .knob { scale: 1.08; }
  .end { color: var(--ink-3); font-size: var(--text-label); }
  .end.r { justify-self: end; grid-column: 2; grid-row: 3; }
  .end.l { grid-column: 1; grid-row: 3; }
  .hint { grid-column: 1 / -1; color: var(--ink-3); font-size: var(--text-label); }
  @media (prefers-reduced-motion: reduce) { .knob, .tick { transition: none; } }
</style>
