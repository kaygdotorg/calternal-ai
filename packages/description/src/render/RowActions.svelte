<script lang="ts">
  // The actions of one row: leading and trailing actions as buttons beside the row (visible on hover, focus and for the selected row), the `menu` as the system menu from a "more" button or the context-menu gesture, and
  // the touch swipe. A swipe reveals nothing the buttons do not already offer; only the FIRST trailing action may be a full swipe, and only where the catalogue allows it for its operation
  // (`full_swipe`), so a swipe never runs a plugin operation or anything that needs a confirmation. Each action's context is the one the host issued with this row's window; none, nothing is sent.
  import { Menu, type MenuItem } from "@calternal/ui";
  import type { Action, RowTemplate, WindowRow } from "../model";
  import type { WindowStore } from "../windows/window-store.svelte";
  import { useActions } from "../invoke/context";
  import { fullSwipeAllowed } from "../invoke/catalogue";
  import { holds } from "./eval";
  import ActionButton from "./ActionButton.svelte";
  import RowView from "./RowView.svelte";
  import ChildRows from "./ChildRows.svelte";
  import { useApp } from "../invoke/context";

  let { template, row, store, kindIcon, selected, depth = 0, onclick }: { template: RowTemplate; row: WindowRow; store: WindowStore; kindIcon?: string; selected: boolean; depth?: number; onclick?: (e: MouseEvent) => void } = $props();
  const app = useApp();
  const kids = $derived(template.children ? app?.storeFor(template.children.bind, row.parent_refs?.[template.children.bind]) : undefined);
  let open = $state(false);
  $effect(() => { if (template.children?.expanded) open = true; });
  const { ui } = useActions();
  const ctx = (a: Action): string | undefined => store.contexts(row.id)?.[a.id];
  const off = (a: Action): boolean => { const d = "disabled" in a ? a.disabled : undefined; return d === true || (typeof d === "object" && d !== null && holds(d, row.fields)); };
  const lead = $derived((template.leadingActions ?? []) as Action[]);
  const trail = $derived((template.trailingActions ?? []) as Action[]);
  const menu = $derived((template.menu ?? []) as Action[]);
  let menuOpen = $state(false), menuAnchor = $state<{ x: number; y: number } | null>(null), moreEl = $state<HTMLElement | null>(null);
  const items = $derived<MenuItem[]>(menu.map((a) => ({ id: a.id, label: a.title ?? a.id, danger: a.role === "destructive", disabled: off(a) })));
  async function pick(id: string) { menuOpen = false; const a = menu.find((m) => m.id === id); if (a && "operation" in a) await ui.invoker.run(a, { context: ctx(a) }); }

  // Swipe (touch only): drag the row left; past 45% of its width the first trailing action runs if it is a catalogue-approved full swipe.
  let dx = $state(0), x0 = 0, w = 1, drag = false;
  const swipe = $derived(trail[0] && "operation" in trail[0] && trail[0].fullSwipe === true && fullSwipeAllowed(trail[0].operation) ? trail[0] : null);
  function down(e: PointerEvent) { if (e.pointerType !== "touch" || !swipe) return; x0 = e.clientX; w = (e.currentTarget as HTMLElement).clientWidth || 1; drag = true; }
  function move(e: PointerEvent) { if (drag) dx = Math.min(0, e.clientX - x0); }
  async function up() {
    if (!drag) return; drag = false;
    const full = swipe && -dx > w * 0.45; dx = 0;
    if (full && swipe) await ui.invoker.run(swipe, { context: ctx(swipe) });
  }
</script>

<div class="ra" data-actions class:sel={selected} oncontextmenu={(e) => { if (menu.length) { e.preventDefault(); menuAnchor = { x: e.clientX, y: e.clientY }; menuOpen = true; } }}
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={() => { drag = false; dx = 0; }} style:transform={dx ? `translateX(${dx}px)` : undefined}>
  <RowView {template} {row} {kindIcon} {selected} {depth} {onclick} />
  {#if template.children && row.parent_refs?.[template.children.bind]}<button type="button" class="disc" aria-expanded={open} aria-label={open ? "Collapse" : "Expand"} onclick={() => (open = !open)}>{open ? "▾" : "▸"}</button>{/if}
  {#if lead.length || trail.length || menu.length}
    <div class="acts" role="group" aria-label="Actions">
      {#each lead as a (a.id)}<ActionButton action={a} context={ctx(a)} disabled={off(a)} />{/each}
      {#each trail as a (a.id)}<ActionButton action={a} context={ctx(a)} disabled={off(a)} />{/each}
      {#if menu.length}<button type="button" class="more" aria-label="More actions" bind:this={moreEl} onclick={() => { menuAnchor = null; menuOpen = true; }}>…</button>{/if}
    </div>
  {/if}
  {#if open && kids}<ChildRows store={kids} depth={depth + 1} {kindIcon} />{/if}
  {#if menu.length}<Menu open={menuOpen} {items} label="Actions" onpick={pick} onclose={() => (menuOpen = false)} trigger={moreEl} anchor={menuAnchor ? { left: menuAnchor.x, top: menuAnchor.y, right: menuAnchor.x, bottom: menuAnchor.y } : null} />{/if}
</div>

<style>
  .ra { position: relative; touch-action: pan-y; }
  .acts { position: absolute; inset-block: 0; inset-inline-end: var(--s-3); display: none; align-items: center; gap: var(--s-2); }
  .ra:hover .acts, .ra:focus-within .acts, .ra.sel .acts { display: flex; }
  .disc { position: absolute; inset-inline-start: 0; top: 0; min-width: 20px; min-height: var(--row-h); color: var(--ink-3); }
  .more { min-width: 44px; min-height: 44px; border-radius: var(--r-select); color: var(--ink-2); }
</style>
