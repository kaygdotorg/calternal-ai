<script lang="ts">
  // The Style sheet: a block (the caret is in one) gets its text style, kind and colour; the note gets three tabs side by side, Text · Page · Background, swiped or chosen from the strip. Ported from the
  // existing app's components/style/StyleSheet.svelte (origin/dev): the same three pages as a row that stops on each (scroll snap), the same tab strip, choosing a tab glides the pages to it (the scroll
  // position jumps and each page slides from where it was on a transform), only the page that shows is built and the others are built when a finger goes to the strip or after two seconds, and the
  // pager is as tall as the page that shows. It sits in the right card until the bottom bar's Style entry (the shell lane) hosts it as the bar's sheet; `target` says which half to show.
  import { Selector } from "@calternal/ui";
  import { tokenEase, tokenMs } from "@calternal/ui";
  import { onMount } from "svelte";
  import BackgroundTab from "./BackgroundTab.svelte";
  import BlockStyle from "./BlockStyle.svelte";
  import PageTab from "./PageTab.svelte";
  import TextTab from "./TextTab.svelte";
  import { noteStyle } from "./store.svelte";

  let { target = $bindable("note"), tab = $bindable("page") }: { target?: "block" | "note"; tab?: "text" | "page" | "bg" } = $props();
  const TABS = [{ value: "text", label: "Text" }, { value: "page", label: "Page" }, { value: "bg", label: "Background" }];
  const WHERE = [{ value: "note", label: "Note" }, { value: "block", label: "Block" }];
  let pager = $state<HTMLElement>();
  let built = $state<Record<string, boolean>>({ [tab]: true });
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let programmatic = false;
  const index = $derived(TABS.findIndex((t) => t.value === tab));

  // The pages not showing are built when a finger or pointer goes to the strip (before the tap lands, so the switch finds them there) or two seconds after the sheet opened.
  const warm = () => { if (!built.text || !built.page || !built.bg) built = { text: true, page: true, bg: true }; };
  onMount(() => { const t = window.setTimeout(warm, 2000); return () => clearTimeout(t); });

  /** Choosing a tab glides the pages there: the scroll position jumps at once and each page starts where it was and slides to rest on a transform (compositor only, never a scroll offset written per frame). */
  function show(v: "text" | "page" | "bg") {
    tab = v;
    built[v] = true;
    const el = pager;
    if (!el) return;
    const to = TABS.findIndex((t) => t.value === v) * el.clientWidth, by = el.scrollLeft - to;
    programmatic = true;
    el.style.scrollSnapType = "none"; // a snap area moves with its page's transform
    el.scrollLeft = to;
    const done = () => { el.style.scrollSnapType = ""; programmatic = false; };
    if (reduce || Math.abs(by) < 1) return void requestAnimationFrame(() => requestAnimationFrame(done));
    const slides = [...el.children].map((pg) => pg.animate([{ translate: `${by}px 0` }, { translate: "0 0" }], { duration: tokenMs("--dur-sheet", 340), easing: tokenEase("--ease-sheet", "cubic-bezier(0.45, 0.6, 0.2, 0.8)") }));
    Promise.all(slides.map((a) => a.finished)).then(done, done);
  }
  function onscroll() {
    if (programmatic || !pager) return;
    const v = TABS[Math.max(0, Math.min(TABS.length - 1, Math.round(pager.scrollLeft / Math.max(1, pager.clientWidth))))]!.value as "text" | "page" | "bg";
    if (v !== tab) { tab = v; built[v] = true; }
  }
  // The pager is as tall as the page that shows (one observer, so a switch reads no layout).
  const heights: number[] = [];
  const fit = () => { const h = heights[index]; if (pager && h) pager.style.height = `${h}px`; };
  $effect(() => {
    if (!pager || target !== "note") return;
    const pages = [...pager.querySelectorAll<HTMLElement>(".pg")];
    const ro = new ResizeObserver((entries) => { for (const e of entries) heights[pages.indexOf(e.target as HTMLElement)] = e.borderBoxSize[0]!.blockSize; fit(); });
    for (const pg of pages) ro.observe(pg);
    pager.scrollLeft = index * pager.clientWidth; // opens on its tab, no glide
    return () => ro.disconnect();
  });
  $effect(() => { void index; fit(); });
</script>

<div class="style-sheet" data-target={target}>
  <div class="strip"><Selector label="Style for" options={WHERE} value={target} onchange={(v) => (target = v as "note" | "block")} /></div>
  {#if noteStyle.readOnly}<p class="ro" role="status">This note is view only, so its style cannot change.</p>{/if}
  {#if target === "block"}
    <BlockStyle />
  {:else}
    <div class="strip" role="presentation" onpointerdown={warm} onfocusin={warm}><Selector label="Style" options={TABS} value={tab} onchange={(v) => show(v as "text" | "page" | "bg")} /></div>
    <div class="pager" bind:this={pager} {onscroll}>
      <div class="pg" aria-hidden={tab !== "text"} inert={tab !== "text"}>{#if built.text}<TextTab />{/if}</div>
      <div class="pg" aria-hidden={tab !== "page"} inert={tab !== "page"}>{#if built.page}<PageTab />{/if}</div>
      <div class="pg" aria-hidden={tab !== "bg"} inert={tab !== "bg"}>{#if built.bg}<BackgroundTab />{/if}</div>
    </div>
  {/if}
</div>

<style>
  .style-sheet { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-6); padding: var(--s-4); }
  .ro { font-size: var(--text-label); color: var(--ink-3); }
  /* The tabs side by side: one row of three pages as wide as the sheet, which stops on each (scroll snap). */
  .pager { display: grid; grid-auto-flow: column; grid-auto-columns: 100%; align-items: start; overflow-x: auto; overflow-y: clip; scroll-snap-type: x mandatory; overscroll-behavior-x: contain; scrollbar-width: none; }
  .pager::-webkit-scrollbar { display: none; }
  .pg { scroll-snap-align: start; scroll-snap-stop: always; min-width: 0; padding: var(--s-2) var(--s-1) var(--s-3); display: grid; align-content: start; gap: var(--s-6); }
  .style-sheet :global(.section-label) { font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-2); margin: 0; }
</style>
