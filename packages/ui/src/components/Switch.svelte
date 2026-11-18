<script lang="ts">
  // A labelled on/off switch, ported from the squish switch (sveltebits.xyz/r/squish-switch) without its animation dependency: the knob rides a spring and stretches along its direction of travel with
  // its speed (squash across, stretch along), swells a little under a mouse, follows a finger or pointer that drags it (it flips as it crosses the middle, releases on the spring with the speed it
  // had, Escape puts it back), and a tap anywhere on the row toggles it. The label is required (it names the switch); `hint` is a second line. Keyboard: the knob is a button (`role=switch`), Space or Enter toggles,
  // the label is a `<label>` for it, the focus ring is the shell's. Reduced motion: no spring, no stretch, no swell. At rest everything is plain CSS (so first paint needs no script); the spring runs on a
  // `transform` only while the knob moves, one rAF loop, and stops at rest. Sizes come from the spacing tokens, so the interface size scales it.
  import { onDestroy } from "svelte";
  import { Spring } from "../lib/spring-step";
  let { label, hint, checked = $bindable(false), disabled = false, onchange }: { label: string; hint?: string; checked?: boolean; disabled?: boolean; onchange?: (v: boolean) => void } = $props();
  const id = $props.id();
  let button = $state<HTMLButtonElement>(), track = $state<HTMLElement>(), thumb = $state<HTMLElement>();
  // The spring of the knob's x (px, 0 = off), and a second, softer spring that follows its speed to drive the stretch.
  const x = new Spring(0, { stiffness: 170, damping: 21.5, mass: 0.9 });
  const flow = new Spring(0, { stiffness: 320, damping: 40, mass: 0.6 });
  const STRETCH = 0.36, MAX_STRETCH = 0.4, STRETCH_SPEED = 600;
  let lastOn = checked, mounted = false, raf = 0, last = 0, target = 0, travel = 0, peak = 1;
  let grip: { id: number; grab: number | null; moved: boolean; startX: number; onAtPress: boolean; slop: number } | null = null;
  let dragging = $state(false), skipClick = false;
  /** The pointer whose drag Escape cancelled: its physical release still comes (and then a click), and must toggle nothing. (The click used to arrive after the one-tick `skipClick` window had closed, so releasing after Escape sometimes flipped the switch again.) */
  let cancelledId: number | null = null;
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  const measure = () => { if (track && thumb) travel = track.offsetWidth - thumb.offsetWidth - 2 * thumb.offsetLeft; return travel; };
  // The speed is taken as if the travel were the original's 38 px (our knob travels about a third of that), so the squish is as visible on a small switch.
  const stretchOf = () => 1 + Math.min(MAX_STRETCH, (Math.abs(flow.value) * (38 / Math.max(8, travel))) / STRETCH_SPEED) * STRETCH;
  const paint = () => {
    if (!thumb) return;
    const s = stretchOf();
    thumb.style.transform = `translateX(${x.value}px) scale(${s}, ${1 / s})`;
  };
  // At rest the CSS position takes over; the largest stretch of the motion that just ended is kept on the knob (`data-peak`) so a test can read what the motion did without having to catch it in a frame.
  const rest = () => { cancelAnimationFrame(raf); raf = 0; if (thumb) { thumb.style.transform = ""; thumb.dataset.peak = peak.toFixed(3); } peak = 1; };
  function tick(now: number) {
    const dt = last ? (now - last) / 1000 : 1 / 60;
    last = now;
    // Both springs advance together in 1/240 s steps, the soft one chasing the knob's speed at every step, so how much it stretches does not depend on how often the screen is drawn (a slow frame must not miss the peak).
    for (let left = Math.min(dt, 0.25); left > 1e-9; left -= 1 / 240) {
      const h = Math.min(left, 1 / 240);
      if (!dragging) x.step(target, h);
      flow.step(x.velocity, h);
      const s = stretchOf();
      if (s > peak) peak = s;
    }
    paint();
    if (!dragging && x.atRest(target) && Math.abs(flow.value) < 1) { x.value = target; rest(); last = 0; return; }
    raf = requestAnimationFrame(tick);
  }
  /** Moves the knob to its state's rest from wherever it is now (a toggle, a release). */
  function go(on: boolean) {
    measure();
    target = on ? travel : 0;
    if (reduced()) { x.value = target; x.velocity = 0; flow.value = 0; rest(); return; }
    if (!raf) { if (x.velocity === 0 && flow.value === 0) x.value = lastOn ? travel : 0; last = 0; raf = requestAnimationFrame(tick); }
    lastOn = on;
  }
  $effect(() => {
    const on = checked;
    if (!mounted) { mounted = true; lastOn = on; return; }
    if (!dragging) go(on);
    lastOn = on;
  });
  const commit = (next: boolean) => { if (next === checked) return; checked = next; onchange?.(next); };
  const localX = (cx: number) => { const r = track!.getBoundingClientRect(); return (cx - r.left) / ((r.width / (track!.offsetWidth || r.width)) || 1); };
  function down(e: PointerEvent) {
    if (disabled || grip || e.button !== 0) return;
    grip = { id: e.pointerId, grab: null, moved: false, startX: e.clientX, onAtPress: checked, slop: e.pointerType === "touch" ? 8 : 4 };
    try { button!.setPointerCapture(e.pointerId); } catch { /* not capturable */ }
  }
  function move(e: PointerEvent) {
    if (!grip || grip.id !== e.pointerId) return;
    measure();
    const lx = localX(e.clientX);
    if (grip.grab === null) { grip.grab = lx - (raf ? x.value : checked ? travel : 0); return; }
    if (!grip.moved && Math.abs(e.clientX - grip.startX) > grip.slop) { grip.moved = true; dragging = true; x.velocity = 0; if (!raf && !reduced()) { last = 0; raf = requestAnimationFrame(tick); } }
    if (!grip.moved) return;
    const nx = Math.min(travel, Math.max(0, lx - grip.grab));
    x.velocity = (nx - x.value) * 60; // px per second, from the last move (about one frame)
    x.value = nx;
    if (reduced()) paintDragOnly();
    commit(nx > travel / 2);
  }
  const paintDragOnly = () => { if (thumb) thumb.style.transform = `translateX(${x.value}px)`; };
  function up(e: PointerEvent, cancelled: boolean) {
    if (!cancelled && cancelledId === e.pointerId) { cancelledId = null; skipClick = true; setTimeout(() => (skipClick = false), 0); return; }
    if (!grip || grip.id !== e.pointerId) return;
    const g = grip; grip = null;
    try { button!.releasePointerCapture(e.pointerId); } catch { /* released already */ }
    if (cancelled) { commit(g.onAtPress); cancelledId = g.id; }
    else if (!g.moved) commit(!checked);
    skipClick = true; setTimeout(() => (skipClick = false), 0);
    const wasDragging = dragging; dragging = false;
    if (wasDragging) go(checked); // a tap is handled by the effect; a drag ends on the spring, with the speed it had
  }
  function click() {
    if (skipClick) { skipClick = false; return; }
    if (!disabled) commit(!checked);
  }
  function key(e: KeyboardEvent) { if (e.key === "Escape" && grip) { const g = grip; up({ pointerId: g.id } as PointerEvent, true); } }
  onDestroy(() => cancelAnimationFrame(raf));
</script>

<div class="sw" class:disabled>
  <label class="tx" id="{id}-l" for={id}><span>{label}</span>{#if hint}<small>{hint}</small>{/if}</label>
  <button {id} bind:this={button} type="button" role="switch" aria-labelledby="{id}-l" aria-checked={checked} {disabled} class="btn" class:held={dragging} onpointerdown={down} onpointermove={move} onpointerup={(e) => up(e, false)} onpointercancel={(e) => up(e, true)} onkeydown={key} onclick={click}>
    <span class="track" bind:this={track}><i class="knob" bind:this={thumb}></i></span>
  </button>
</div>

<style>
  .sw { --tw: calc(var(--s-9) + var(--s-4)); --th: calc(var(--s-8) + var(--s-1) * 2); --tk: var(--s-8); --ti: var(--s-1); box-sizing: border-box; min-width: 0; display: flex; align-items: center; justify-content: space-between; gap: var(--s-5); width: 100%; min-height: var(--pill-h); }
  .sw.disabled { opacity: 0.5; }
  .tx { display: grid; flex: 1; min-width: 0; cursor: pointer; align-self: stretch; align-content: center; text-align: start; font-size: var(--text-ui); }
  .sw.disabled .tx { cursor: not-allowed; }
  small { font-size: var(--text-label); color: var(--ink-3); }
  .btn { position: relative; flex: none; border: 0; padding: 0; margin: 0; background: transparent; border-radius: var(--r-pill); touch-action: pan-y; cursor: pointer; -webkit-tap-highlight-color: transparent; user-select: none; }
  .btn:disabled { cursor: not-allowed; }
  .btn.held { cursor: grabbing; }
  /* A bigger target than the track (44 px on touch comes from the row). */
  .btn::after { content: ""; position: absolute; inset: calc(var(--s-2) * -1); }
  .track { position: relative; display: block; width: var(--tw); height: var(--th); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.18); transition: background-color var(--dur-base) ease; }
  .btn[aria-checked="true"] .track { background: var(--accent-base); }
  .knob { position: absolute; left: var(--ti); top: var(--ti); width: var(--tk); height: var(--tk); border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgb(0 0 0 / 0.3); transform-origin: center; transition: scale var(--dur-base) var(--spring); }
  /* At rest the position is CSS alone (the spring only runs while it moves). */
  .btn[aria-checked="true"] .knob { transform: translateX(calc(var(--tw) - var(--tk) - var(--ti) * 2)); }
  @media (hover: hover) and (pointer: fine) { .btn:not(:disabled):hover .knob { scale: 1.035; } }
  @media (prefers-reduced-motion: reduce) { .track { transition-duration: 1ms; } .knob { transition: none; scale: none !important; } }
</style>
