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
  // The keyboard and context menu offers every action of the row: the swipe actions too (a swipe is never the only way), then the declared menu, each once.
  const every = $derived.by(() => { const seen = new Set<string>(); return [...lead, ...trail, ...menu].filter((a) => !seen.has(a.id) && !!seen.add(a.id)); });
  const items = $derived<MenuItem[]>(every.map((a) => ({ id: a.id, label: a.title ?? a.id, danger: a.role === "destructive", disabled: off(a) })));
  async function pick(id: string) { menuOpen = false; const a = every.find((m) => m.id === id); if (a && "operation" in a) await ui.invoker.run(a, { context: ctx(a) }); }

  // Swipe (touch only): dragging a row right shows its leading actions, left its trailing ones; past 45% of the row's width the FIRST action of that side runs, but only if it is a full swipe the catalogue allows for its
  // operation (`full_swipe`). The same actions are buttons and menu items, so a swipe is never the only way, and a swipe never opens a confirmation or runs a plugin operation.
  let dx = $state(0), x0 = 0, y0 = 0, w = 1, drag = false;
  const fullOf = (a: Action | undefined) => (a && "operation" in a && a.fullSwipe === true && fullSwipeAllowed(a.operation) ? a : null);
  const swipeRight = $derived(fullOf(lead[0])), swipeLeft = $derived(fullOf(trail[0]));
  function down(e: PointerEvent) { if (e.pointerType !== "touch" || (!swipeRight && !swipeLeft)) return; x0 = e.clientX; y0 = e.clientY; w = (e.currentTarget as HTMLElement).clientWidth || 1; drag = true; }
  function move(e: PointerEvent) { if (!drag) return; const d = e.clientX - x0; if (Math.abs(e.clientY - y0) > Math.abs(d) * 1.5 && Math.abs(d) < 12) return; dx = d > 0 ? (swipeRight ? d : 0) : swipeLeft ? d : 0; }
  async function up() {
    if (!drag) return; drag = false;
    const act = dx > w * 0.45 ? swipeRight : -dx > w * 0.45 ? swipeLeft : null; dx = 0;
    if (act) await ui.invoker.run(act, { context: ctx(act) });
  }
</script>

<div class="ra" data-actions class:sel={selected} oncontextmenu={(e) => { if (every.length) { e.preventDefault(); menuAnchor = { x: e.clientX, y: e.clientY }; menuOpen = true; } }}
  onkeydown={(e) => { if (template.children && (e.target as HTMLElement).matches?.("button.row")) { if (e.key === "ArrowRight" && !open) { open = true; e.preventDefault(); } else if (e.key === "ArrowLeft" && open) { open = false; e.preventDefault(); } } }}
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={() => { drag = false; dx = 0; }} style:transform={dx ? `translateX(${dx}px)` : undefined}>
  <RowView {template} {row} {kindIcon} {selected} {depth} {onclick} />
  {#if template.children && row.parent_refs?.[template.children.bind]}<button type="button" tabindex="-1" class="disc" aria-expanded={open} aria-label={open ? "Collapse" : "Expand"} onclick={() => (open = !open)}>{open ? "▾" : "▸"}</button>{/if}
  {#if lead.length || trail.length || menu.length}
    <div class="acts" role="group" aria-label="Actions">
      {#each lead as a (a.id)}<ActionButton action={a} context={ctx(a)} disabled={off(a)} />{/each}
      {#each trail as a (a.id)}<ActionButton action={a} context={ctx(a)} disabled={off(a)} />{/each}
      {#if every.length}<button type="button" tabindex="-1" class="more" aria-label="More actions" bind:this={moreEl} onclick={() => { menuAnchor = null; menuOpen = true; }}>…</button>{/if}
    </div>
  {/if}
  {#if open && kids}<ChildRows store={kids} depth={depth + 1} {kindIcon} />{/if}
  {#if every.length}<Menu open={menuOpen} {items} label="Actions" onpick={pick} onclose={() => (menuOpen = false)} trigger={moreEl} anchor={menuAnchor ? { left: menuAnchor.x, top: menuAnchor.y, right: menuAnchor.x, bottom: menuAnchor.y } : null} />{/if}
</div>

<style>
  .ra { position: relative; touch-action: pan-y; }
  /* Row actions are compact: an icon where the action has one (its title is the accessible name and the tooltip), so the group never covers the row's own label, and only its buttons take pointer events. */
  .acts { pointer-events: none; position: absolute; inset-block: 0; inset-inline-end: var(--s-3); display: flex; align-items: center; gap: var(--s-2); opacity: 0; }
  /* Hidden by opacity, not display, so the arrow keys can move focus into it; its buttons take pointer events only while it is shown. */
  .ra:hover .acts, .ra:focus-within .acts, .ra.sel .acts { opacity: 1; }
  .disc { position: absolute; inset-inline-start: 0; top: 0; min-width: 20px; min-height: var(--row-h); color: var(--ink-3); }
  .ra:hover .acts :global(button), .ra:focus-within .acts :global(button), .ra.sel .acts :global(button) { pointer-events: auto; }
  .acts :global(.btn[data-has-icon]) { padding: 0 var(--s-3); min-width: 44px; justify-content: center; }
  .acts :global(.btn[data-has-icon] > span:last-child) { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  .more { min-width: 44px; min-height: 44px; border-radius: var(--r-select); color: var(--ink-2); }
</style>
