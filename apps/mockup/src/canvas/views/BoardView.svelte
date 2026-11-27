<script lang="ts">
  // The board (kanban): one column per status with its name and count, and a real card per item drawn in the look of its kind. Drag a card to another column to change its status (a mouse after 5 px; touch after a half-second hold, then the page
  // does not scroll); the column under the card is outlined, the card follows the pointer, and releasing sets the status. The columns scroll on their own so a long column never pushes the others. Escape cancels. Motion: the ghost is the pointer's own,
  // the dropped card settles into place with a short rise (`--dur-base`, `--ease-out`); reduced motion makes it instant.
  import { tokenEase } from "@calternal/ui";
  import { ms } from "../../motion";
  import Item from "../../shell/Item.svelte";
  import { arr } from "../../arrange.svelte";
  import type { Item as ItemT } from "../../data/collections";
  let { items, onopen }: { items: ItemT[]; onopen: (i: ItemT) => void } = $props();
  const COLS = ["To do", "Doing", "Done"] as const;
  const st = (i: ItemT) => arr.statusOf(i.id) ?? COLS[[...i.id].reduce((a, c) => a + c.charCodeAt(0), 0) % 3]!;
  const by = $derived(Object.fromEntries(COLS.map((c) => [c, items.filter((i) => st(i) === c)])) as Record<string, ItemT[]>);
  let drag = $state<{ item: ItemT; x: number; y: number; over: string | null } | null>(null), pend: { item: ItemT; x: number; y: number; pid: number; t: number } | null = null, quiet = false;
  let cols: HTMLElement[] = [];
  function down(e: PointerEvent, item: ItemT) {
    if (e.button !== 0 || pend) return; pend = { item, x: e.clientX, y: e.clientY, pid: e.pointerId, t: 0 };
    addEventListener("pointermove", move); addEventListener("pointerup", up); addEventListener("pointercancel", stop); addEventListener("keydown", key);
    if (e.pointerType !== "mouse") pend.t = window.setTimeout(() => pend && start(pend.x, pend.y), 450);
  }
  function start(x: number, y: number) { if (!pend) return; clearTimeout(pend.t); drag = { item: pend.item, x, y, over: null }; addEventListener("touchmove", ns, { passive: false }); document.documentElement.style.userSelect = "none"; navigator.vibrate?.(8); }
  const ns = (e: TouchEvent) => { if (drag) e.preventDefault(); };
  function move(e: PointerEvent) {
    if (!pend || e.pointerId !== pend.pid) return;
    if (!drag) { if (Math.hypot(e.clientX - pend.x, e.clientY - pend.y) < (e.pointerType === "mouse" ? 5 : 8)) return; if (e.pointerType !== "mouse") return stop(); start(e.clientX, e.clientY); }
    const d = drag!; d.x = e.clientX; d.y = e.clientY; d.over = cols.find((c) => { const b = c.getBoundingClientRect(); return e.clientX >= b.left && e.clientX <= b.right && e.clientY >= b.top && e.clientY <= b.bottom; })?.dataset.col ?? null;
  }
  function up(e: PointerEvent) { if (!pend || e.pointerId !== pend.pid) return; const d = drag; stop(); if (d) { quiet = true; setTimeout(() => (quiet = false), 0); if (d.over && d.over !== st(d.item)) arr.setStatus(d.item.id, d.over); } }
  const key = (e: KeyboardEvent) => e.key === "Escape" && stop();
  function stop() { removeEventListener("pointermove", move); removeEventListener("pointerup", up); removeEventListener("pointercancel", stop); removeEventListener("keydown", key); removeEventListener("touchmove", ns); if (pend) clearTimeout(pend.t); pend = null; drag = null; document.documentElement.style.userSelect = ""; }
  const open = (i: ItemT) => { if (!quiet) onopen(i); };
  const rise = (n: HTMLElement) => { n.animate([{ opacity: 0, translate: "0 8px" }, { opacity: 1, translate: "0 0" }], { duration: ms("--dur-base", 220), easing: tokenEase("--ease-out", "cubic-bezier(0.23, 1, 0.32, 1)") }); };
</script>

<div class="board" data-board>
  {#each COLS as c, i (c)}
    <section class="col" class:over={drag?.over === c} data-col={c} bind:this={cols[i]} aria-label={`${c}, ${by[c]!.length} items`}>
      <header><strong>{c}</strong><span class="n">{by[c]!.length}</span></header>
      <div class="cards">
        {#each by[c]! as it (it.id)}<div class="cw" class:held={drag?.item.id === it.id} onpointerdown={(e) => down(e, it)} use:rise><Item item={{ ...it, status: st(it) }} mode="card" onopen={open} /></div>{/each}
        {#if !by[c]!.length}<p class="em">No cards</p>{/if}
      </div>
    </section>
  {/each}
  {#if drag}<div class="ghost" style:left="{drag.x - 90}px" style:top="{drag.y - 24}px"><Item item={drag.item} mode="card" /></div>{/if}
</div>

<style>
  .board { display: grid; grid-template-columns: repeat(3, minmax(13rem, 1fr)); gap: var(--s-5); height: 100%; min-height: 0; align-items: stretch; }
  .col { display: flex; flex-direction: column; min-height: 0; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); transition: background-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out); } .col.over { background: var(--accent-soft); box-shadow: inset 0 0 0 2px var(--accent); }
  header { flex: none; display: flex; align-items: center; gap: var(--s-4); padding: var(--s-5) var(--s-5) var(--s-3); font-size: var(--text-ui); } header strong { font-weight: var(--strong-weight); } .n { min-width: 20px; padding: 0 var(--s-3); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.1); color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; text-align: center; }
  .cards { flex: 1; min-height: 0; overflow-y: auto; display: grid; align-content: start; gap: var(--s-3); padding: var(--s-2) var(--s-3) var(--s-4); scrollbar-width: none; } .cw { touch-action: pan-y; } .cw.held { opacity: 0.35; }
  .em { margin: var(--s-4); color: var(--ink-3); font-size: var(--text-label); } .ghost { position: fixed; z-index: 95; width: 180px; pointer-events: none; rotate: 2deg; box-shadow: 0 12px 32px rgb(0 0 0 / 0.3); border-radius: var(--r-menu); }
  @media (max-width: 719px) { .board { grid-template-columns: repeat(3, 15rem); overflow-x: auto; } }
</style>
