<script lang="ts">
  // THE app switcher: one capsule of the apps' names (text only, no icons), three of them in view, the rest reached by scrolling it sideways (no "More" list or menu), in the order the person chose (Settings, Appearance,
  // Apps). One component with two placements: under the middle card on a wide screen (`card`) and as the bottom bar on a phone (`bar`, riding with the card when the list or panel is pulled), so it is the same control
  // everywhere and a tap switches the app. The chosen app is always scrolled into view. It is a tab list: the arrow keys move and choose, only the chosen name is in the Tab order. Scrolling snaps a name at a time;
  // a fade at an edge says there is more that way. Spaces are not here (they sit behind the account picture).
  import { Panel } from "@calternal/ui";
  import { appearance } from "../appearance/prefs.svelte";
  import { APPS, type AppDef } from "../apps/registry";
  import { navigate } from "../lib/route.svelte";
  import { applyOrder } from "./appOrder";
  let { app, placement = "card" }: { app: AppDef; placement?: "card" | "bar" } = $props();
  const apps = $derived(applyOrder(APPS, appearance.prefs.appOrder));
  let track = $state<HTMLElement>(), more = $state({ left: false, right: false });
  const measure = () => { if (!track) return; more = { left: track.scrollLeft > 2, right: track.scrollLeft + track.clientWidth < track.scrollWidth - 2 }; };
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.input === "key";
  // The chosen app comes into view (centred), gliding unless the person asked for less motion or used the keyboard.
  $effect(() => {
    void app.kind; void apps.length;
    const el = track?.querySelector<HTMLElement>('[aria-selected="true"]');
    if (!track || !el) return;
    track.scrollTo({ left: el.offsetLeft - (track.clientWidth - el.offsetWidth) / 2, behavior: reduced() ? "instant" : "smooth" });
    queueMicrotask(measure);
  });
  function key(e: KeyboardEvent) {
    const i = apps.findIndex((a) => a.kind === app.kind);
    const to = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? apps.length - 1 : null;
    if (to === null || to < 0 || to >= apps.length) return;
    e.preventDefault();
    navigate(`/${apps[to]!.kind}`);
    queueMicrotask(() => track?.querySelector<HTMLElement>('[aria-selected="true"]')?.focus());
  }
</script>

<nav class="switcher {placement}" aria-label="Apps">
  <Panel class="sw" radius="pill" dense>
    <!-- svelte-ignore a11y_interactive_supports_focus (the tabs inside are the focusable parts, as in any tab list) -->
    <div class="track" role="tablist" aria-label="Apps" bind:this={track} onscroll={measure} onkeydown={key} data-more-left={more.left || undefined} data-more-right={more.right || undefined}>
      {#each apps as a (a.kind)}
        <button type="button" role="tab" class="app" aria-selected={a.kind === app.kind} tabindex={a.kind === app.kind ? 0 : -1} onclick={() => navigate(`/${a.kind}`)}>{a.name}</button>
      {/each}
    </div>
  </Panel>
</nav>

<style>
  .switcher { display: flex; justify-content: center; width: 100%; }
  .switcher :global(.sw) { width: 100%; max-width: 21rem; }
  .track { display: flex; overflow-x: auto; scroll-snap-type: x mandatory; scrollbar-width: none; overscroll-behavior-x: contain; padding: var(--s-2); border-radius: var(--r-pill); }
  .track::-webkit-scrollbar { display: none; }
  /* A fade at the side that has more, as wide as half a name. */
  .track[data-more-left] { mask-image: linear-gradient(90deg, transparent, #000 var(--s-9)); }
  .track[data-more-right] { mask-image: linear-gradient(270deg, transparent, #000 var(--s-9)); }
  .track[data-more-left][data-more-right] { mask-image: linear-gradient(90deg, transparent, #000 var(--s-9), #000 calc(100% - var(--s-9)), transparent); }
  /* Three names fill the capsule; the rest wait beyond its edge. */
  .app { flex: 0 0 calc((100% - var(--s-2) * 0) / 3); scroll-snap-align: center; min-height: calc(var(--pill-h) - var(--s-4)); padding: 0 var(--s-4); border-radius: var(--r-pill); color: var(--ink-2); font-size: var(--text-ui); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  .app[aria-selected="true"] { background: rgb(var(--ink-rgb) / 0.12); color: var(--ink); font-weight: var(--strong-weight); }
  .app:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .app[aria-selected="false"]:hover { color: var(--ink); } }
  /* The phone's bottom bar: fixed at the bottom, riding with the card (the shell sets --pull-x while the list or the panel is pulled), above the keyboard. */
  .switcher.bar { position: fixed; z-index: 5; left: var(--s-6); right: var(--s-6); bottom: calc(var(--cap-offset) + var(--kb, 0px)); width: auto; transform: var(--pull-x, none); transition: transform var(--dur-sheet) var(--ease-sheet); }
  :global(.shell:has(.mid.dragging)) .switcher.bar { transition: none; }
  @media (prefers-reduced-motion: reduce) { .track { scroll-behavior: auto; } .app { transition: none; } }
</style>
