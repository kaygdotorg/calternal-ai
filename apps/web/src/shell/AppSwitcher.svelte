<script lang="ts">
  // THE app switcher: an icon-only capsule at the bottom centre of the left sidebar (footer order: account picture, this, search), three icons at a time, the rest reached by scrolling it sideways; a click or tap
  // switches app. It is a `nav` of real links (Enter opens, "Threads, link, current page" is announced, the address is right-clickable); every icon has its name as its accessible name and as a tooltip (hover,
  // keyboard focus, long press). One tab stop (the current app); Left, Right, Home and End move focus and bring that icon to the middle. The current app is always in view: centred on mount and whenever the app changes by
  // any route. The strip shows exactly three whole icons; when more apps exist on a side, the outer icon on that side is smaller and fainter (the page-indicator pattern, no clipped icon, no gradient); with three or fewer
  // all are full size and nothing scrolls. Scrolling snaps one icon at a time. Motion (tokens): the current marker and the outer icons' scale and opacity over `--dur-base` on `--ease-out`; press `--press-scale`; centring after a
  // key or an outside change is an animation of `scrollLeft` over `--dur-base` on `--ease-out`, cancelled by the next touch, wheel or key (interruptible), instant under reduced motion.
  import { Icon, tip } from "@calternal/ui";
  import { navigate } from "../lib/route.svelte";
  import { letterOf, switcherApps, type SwitcherApp } from "./switcherApps.svelte";
  let { current }: { current: string } = $props();
  const apps = $derived(switcherApps());
  let track = $state<HTMLElement>(), at = $state(0); // `at`: the index of the icon nearest the middle
  const slot = () => (track && track.children[0] ? (track.children[0] as HTMLElement).offsetWidth : 40);
  const middleOf = (i: number) => i * slot(); // scrollLeft that puts icon i in the middle slot (the track is padded by one slot at each end)
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.input === "key";
  let glideId = 0;
  /** Animates the track to `left` over the base duration on the out curve; cancelled by any later gesture. */
  function glide(left: number) {
    if (!track) return;
    const cs = getComputedStyle(document.documentElement);
    const ms = parseFloat(cs.getPropertyValue("--dur-base")) || 220;
    cancelAnimationFrame(glideId);
    if (reduced() || ms <= 1) { track.scrollLeft = left; return; }
    const from = track.scrollLeft, t0 = performance.now();
    const ease = (t: number) => 1 - Math.pow(1 - t, 4); // an out curve; the exact token curve is cubic-bezier(.23,1,.32,1)
    const step = (now: number) => { if (!track) return; const t = Math.min(1, (now - t0) / ms); track.scrollLeft = from + (left - from) * ease(t); if (t < 1) glideId = requestAnimationFrame(step); };
    glideId = requestAnimationFrame(step);
  }
  const stop = () => cancelAnimationFrame(glideId);
  const indexOfCurrent = () => Math.max(0, apps.findIndex((a) => a.kind === current));
  // Centred on mount (no animation) and whenever the current app or the list changes.
  let first = true;
  $effect(() => {
    void current; void apps.length;
    const i = indexOfCurrent();
    queueMicrotask(() => { if (!track) return; if (first) { track.scrollLeft = middleOf(i); first = false; } else glide(middleOf(i)); measure(); });
  });
  function measure() { if (!track) return; at = Math.max(0, Math.min(apps.length - 1, Math.round(track.scrollLeft / slot()))); }
  // A side has more apps when the icon beyond the visible three exists on it; the outer visible icon on that side is then drawn smaller and fainter.
  const outer = (i: number): boolean => apps.length > 3 && ((i === at - 1 && at - 1 > 0) || (i === at + 1 && at + 1 < apps.length - 1));
  function key(e: KeyboardEvent) {
    const cur = apps.findIndex((a) => a.kind === (document.activeElement as HTMLElement | null)?.dataset.kind);
    const from = cur < 0 ? indexOfCurrent() : cur;
    const to = e.key === "ArrowRight" ? from + 1 : e.key === "ArrowLeft" ? from - 1 : e.key === "Home" ? 0 : e.key === "End" ? apps.length - 1 : null;
    if (to === null || to < 0 || to >= apps.length) return;
    e.preventDefault(); stop();
    glide(middleOf(to));
    queueMicrotask(() => track?.querySelector<HTMLElement>(`[data-kind="${apps[to]!.kind}"]`)?.focus({ preventScroll: true }));
  }
  const go = (e: MouseEvent, a: SwitcherApp) => { if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return; e.preventDefault(); navigate(`/${a.kind}`); };
</script>

<nav class="switcher" aria-label="Apps" data-count={apps.length}>
  <div class="track" bind:this={track} onscroll={measure} onkeydown={key} onpointerdown={stop} onwheel={stop} role="presentation">
    {#each apps as a, i (a.kind)}
      <a class="app" class:cur={a.kind === current} class:outer={outer(i)} href="/{a.kind}" data-kind={a.kind} aria-label={a.name} aria-current={a.kind === current ? "page" : undefined} tabindex={a.kind === current ? 0 : -1} use:tip={{ label: a.name }} onclick={(e) => go(e, a)}>
        {#if a.icon}<Icon name={a.icon} />{:else if a.iconPath}<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" aria-hidden="true"><path d={a.iconPath} /></svg>{:else}<span class="letter" aria-hidden="true">{letterOf(a.name)}</span>{/if}
      </a>
    {/each}
  </div>
</nav>

<style>
  /* Three slots wide; the track is padded by one slot at each end so the first and last icons can be centred too. */
  .switcher { --slot: var(--pill-h); min-width: 0; display: flex; justify-content: center; }
  .track { display: flex; width: calc(var(--slot) * 3); max-width: 100%; overflow-x: auto; scroll-snap-type: x mandatory; scrollbar-width: none; overscroll-behavior-x: contain; padding-inline: var(--slot); box-sizing: border-box; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); }
  .switcher[data-count="1"] .track, .switcher[data-count="2"] .track, .switcher[data-count="3"] .track { overflow-x: visible; padding-inline: 0; width: auto; justify-content: center; scroll-snap-type: none; }
  .track::-webkit-scrollbar { display: none; }
  .app { flex: none; width: var(--slot); height: var(--slot); display: grid; place-items: center; scroll-snap-align: center; border-radius: var(--r-pill); color: var(--ink-2); position: relative; isolation: isolate; transition: scale var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out), color var(--dur-base) var(--ease-out); }
  /* The marker behind the current icon fades and scales in on the tokens. */
  .app::before { content: ""; position: absolute; inset: var(--s-1); z-index: -1; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.14); opacity: 0; scale: 0.8; transition: opacity var(--dur-base) var(--ease-out), scale var(--dur-base) var(--ease-out); }
  .app.cur { color: var(--ink); }
  .app.cur::before { opacity: 1; scale: 1; }
  .app.outer { scale: 0.75; opacity: 0.55; }
  .app:active { scale: var(--press-scale); }
  .letter { display: grid; place-items: center; width: var(--glyph); height: var(--glyph); border-radius: 50%; border: 1.5px solid currentColor; font-size: calc(var(--glyph) * 0.6); font-weight: var(--strong-weight); line-height: 1; }
  svg { width: var(--glyph); height: var(--glyph); stroke-width: var(--glyph-stroke); stroke-linecap: round; stroke-linejoin: round; }
  @media (hover: hover) and (pointer: fine) { .app:not(.cur):hover { color: var(--ink); } }
  @media (prefers-reduced-motion: reduce) { .app, .app::before { transition: none; } }
</style>
