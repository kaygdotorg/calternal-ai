<script lang="ts">
  // THE app switcher: an icon-only capsule at the bottom centre of the left sidebar (footer order: account picture, this, search), three icons at a time, the rest reached by scrolling it sideways; a click or tap switches app.
  // It is the shared Selector (packages/ui: an outer pill with the chosen option as a pill inside it, arrow keys, Home and End, one tab stop, a tooltip with each app's name), sized to three icons; this component only adds what an
  // app list needs: scrolling, keeping the current app in view, and the hint that more apps exist. The current app is centred when it can be (at an end the strip rests flush) on mount and whenever it changes by any route (centring
  // is an animation of `scrollLeft` over `--dur-base` on an out curve, cancelled by the next touch, wheel or key, instant under reduced motion). The strip shows exactly three whole icons; when more apps exist on a side, the outer
  // icon on that side is drawn smaller and fainter (the page-indicator pattern: no clipped icon, no gradient); with three or fewer all are full size and nothing scrolls. Apps from plugins are data (switcherApps.svelte.ts).
  import { Icon, Selector, tip, tokenMs } from "@calternal/ui";
  import { navigate } from "../lib/route.svelte";
  import { letterOf, switcherApps } from "./switcherApps.svelte";
  let { current }: { current: string } = $props();
  const apps = $derived(switcherApps());
  const options = $derived(apps.map((a) => ({ value: a.kind, label: a.name, iconOnly: true })));
  // The Selector marks the chosen app itself; this copy follows the address (a choice made elsewhere, the palette, Back).
  // svelte-ignore state_referenced_locally
  let chosen = $state(current);
  $effect(() => { chosen = current; });
  let root = $state<HTMLElement>(), at = $state(1); // `at`: the index of the app in the middle slot (the three in view are at-1, at, at+1)
  const scroller = () => root?.querySelector<HTMLElement>(".selector") ?? null;
  const pitch = () => { const o = scroller()?.querySelectorAll<HTMLElement>(".opt"); return o && o.length > 1 ? o[1]!.offsetLeft - o[0]!.offsetLeft : 40; };
  /** The scrollLeft that puts app `i` in the middle slot, held inside the strip: the first and last cannot be centred, so at the ends the strip rests flush. */
  const middleOf = (i: number) => Math.max(0, Math.min((apps.length - 3) * pitch(), (i - 1) * pitch()));
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.input === "key";
  let glideId = 0;
  function glide(left: number) {
    const el = scroller();
    if (!el) return;
    const ms = tokenMs("--dur-base", 220);
    el.dataset.glideMs = String(ms); // read by e2e/app-switcher.mjs: the glide is a scroll, which Element.animate cannot record
    cancelAnimationFrame(glideId);
    if (reduced() || ms <= 1) { el.scrollLeft = left; return; }
    const from = el.scrollLeft, t0 = performance.now();
    const step = (now: number) => { const t = Math.min(1, (now - t0) / ms); el.scrollLeft = from + (left - from) * (1 - Math.pow(1 - t, 4)); if (t < 1) glideId = requestAnimationFrame(step); };
    glideId = requestAnimationFrame(step);
  }
  const stop = () => cancelAnimationFrame(glideId);
  let first = true;
  $effect(() => {
    void current; void apps.length;
    const i = Math.max(0, apps.findIndex((a) => a.kind === current));
    queueMicrotask(() => { const el = scroller(); if (!el) return; if (first) { el.scrollLeft = middleOf(i); first = false; } else glide(middleOf(i)); measure(); });
  });
  function measure() { const el = scroller(); if (el) at = Math.max(1, Math.min(apps.length - 2, Math.round(el.scrollLeft / pitch()) + 1)); }
  $effect(() => { const el = scroller(); if (!el) return; el.addEventListener("scroll", measure, { passive: true }); el.addEventListener("pointerdown", stop); el.addEventListener("wheel", stop, { passive: true }); return () => { el.removeEventListener("scroll", measure); el.removeEventListener("pointerdown", stop); el.removeEventListener("wheel", stop); }; });
  const outer = (kind: string): boolean => { const i = apps.findIndex((a) => a.kind === kind); return apps.length > 3 && ((i === at - 1 && at - 1 > 0) || (i === at + 1 && at + 1 < apps.length - 1)); };
</script>

<nav class="switcher" aria-label="Apps" bind:this={root} data-count={apps.length}>
  <Selector label="Apps" bind:value={chosen} {options} size="lg" fill={false} class="apps" tooltip={tip} onpick={(k) => navigate(`/${k}`)}>
    {#snippet optionIcon(o)}
      {@const a = apps.find((x) => x.kind === o.value)}
      <span class="ic" class:outer={outer(o.value)}>
        {#if a?.icon}<Icon name={a.icon} />{:else if a?.iconPath}<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" aria-hidden="true"><path d={a.iconPath} /></svg>{:else}<span class="letter" aria-hidden="true">{letterOf(o.label)}</span>{/if}
      </span>
    {/snippet}
  </Selector>
</nav>

<style>
  .switcher { min-width: 0; display: flex; justify-content: center; }
  /* Three icons wide (the Selector scrolls sideways by itself); with three or fewer apps it is as wide as they are. */
  .switcher :global(.apps) { max-width: calc(var(--pill-h) * 3 + 8px); }
  .ic { display: grid; place-items: center; transition: scale var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out); }
  .ic.outer { scale: 0.75; opacity: 0.55; }
  .letter { display: grid; place-items: center; width: var(--glyph); height: var(--glyph); border-radius: 50%; border: 1.5px solid currentColor; font-size: calc(var(--glyph) * 0.6); font-weight: var(--strong-weight); line-height: 1; }
  svg { width: var(--glyph); height: var(--glyph); stroke-width: var(--glyph-stroke); stroke-linecap: round; stroke-linejoin: round; }
  @media (prefers-reduced-motion: reduce) { .ic { transition: none; } }
</style>
