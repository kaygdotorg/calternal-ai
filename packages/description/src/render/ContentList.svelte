<script lang="ts">
  // A bound list: the windowed rows of one bind drawn with its row template, and its states. It reuses the design system's VirtualList (the only windowed list; one primitive, not forked) and Row, and the store
  // keeps the host's windows. While loading it draws skeleton rows of the real row height; `empty`, `error` and `refine_query` show the bind's own message (the renderer's plain default only for `error` and
  // `refine_query`, which a description may leave unsaid). The list takes the height of its container and scrolls itself, or with `inside` follows the nearest scrolling ancestor (a sidebar).
  import { getContext, untrack } from "svelte";
  import { ACTIONS_KEY, APP_KEY, type ActionContext, type AppContext } from "../invoke/context";
  import { VirtualList, cssPx } from "@calternal/ui";
  import type { Bind, State } from "../model";
  import type { WindowStore } from "../windows/window-store.svelte";
  import type { Selection } from "../windows/selection.svelte";
  import RowActions from "./RowActions.svelte";
  import SkeletonRow from "./SkeletonRow.svelte";
  import StateMessage from "./StateMessage.svelte";

  let { store, selection, kindIcon, label, inside = false, multiple = false, onopen }: {
    store: WindowStore; selection: Selection; kindIcon?: string; label: string; inside?: boolean; /** `selectionMode: multiple`: rows carry a tick box. */ multiple?: boolean;
    /** The selection settled on a row (after a key burst, or a click): the route opens here. Called once per settle. */
    onopen?: (id: string, index: number) => void;
  } = $props();
  let vl = $state<{ scrollToIndex(i: number, a?: "centre" | "start"): void }>();
  let wantEnd = $state(false);
  // End on a list whose size is not known pages forward; when the last window has arrived the list scrolls to its bottom.
  $effect(() => { void store.tick; if (wantEnd && store.walkGoal === null && vl && store.length > 0) { wantEnd = false; vl.scrollToIndex(store.length - 1, "start"); } });
  const app = getContext<AppContext | undefined>(APP_KEY), actions = getContext<ActionContext | undefined>(ACTIONS_KEY);
  /**
   * The row was chosen: one settled selection, then what the row opens. A route parameter opens on every choice (the contract: the route opens when the selection settles). An `open.invoke` action runs ONLY on an
   * explicit activation (click, Enter), never when the cursor merely rests on the row after arrowing: the contract does not make a settled cursor an activation (reported as a gap for revision 14).
   */
  const choose = (id: string, i: number, explicit: boolean) => {
    void selection.settle(id); onopen?.(id, i);
    const o = template.open;
    if (o && "route" in o) app?.setRoute(o.route.name, id);
    else if (explicit && o && "invoke" in o && "operation" in o.invoke) void actions?.ui.invoker.run(o.invoke, { context: store.contexts(id)?.[o.invoke.id] });
  };

  // The rows may no longer be the ones the selection was made on (the query changed, an action or Undo changed the list): the selection is cleared and the cursor row, if it is still there, is settled again so its
  // contexts are the fresh ones; the ticked set is dropped.
  let again: string | null = null;
  $effect(() => { void store.generation; untrack(() => { again = selection.drawn; selection.clear(); }); });
  $effect(() => { void store.tick; untrack(() => { if (again && store.indexOf(again) >= 0) { const id = again; again = null; void selection.settle(id); } }); });

  // One Tab stop per list: the controls inside a row (action buttons, the more button, the disclosure, the tick box) are reached by keys, never by Tab; the list itself (its scroller, or with `inside` the selected or first
  // row) is the one stop.
  let root = $state<HTMLElement>();
  function rove() {
    if (!root) return;
    let stop: HTMLElement | null = null, firstMain: HTMLElement | null = null;
    for (const row of root.querySelectorAll<HTMLElement>("[data-i]")) {
      const main = row.querySelector<HTMLElement>("button.row");
      for (const c of row.querySelectorAll<HTMLElement>("button, input, a, select, textarea")) c.tabIndex = -1;
      if (main) { firstMain ??= main; if (!stop && row.querySelector("[aria-current]")) stop = main; }
    }
    if (inside) (stop ?? firstMain)?.setAttribute("tabindex", "0");
  }
  $effect(() => {
    void selection.drawn; void store.tick;
    if (!root) return;
    rove();
    const mo = new MutationObserver(() => rove());
    mo.observe(root, { childList: true, subtree: true });
    return () => mo.disconnect();
  });
  const onkey = (e: KeyboardEvent) => {
    if (!multiple || e.key !== " " || !(e.target as HTMLElement).matches?.("button.row")) return;
    const i = Number((e.target as HTMLElement).closest<HTMLElement>("[data-i]")?.dataset.i); const r = store.row(i);
    if (r) { e.preventDefault(); selection.toggle(r.id); } // Space ticks in a multiple-selection list; Enter opens
  };

  const bind: Bind = $derived(store.bind);
  const template = $derived(bind.row ?? { title: { field: "title" } });
  const consumer = {};
  // (untracked: the store's own state must not re-run this, or each reply would forget what the list draws)
  $effect(() => { const s = store; untrack(() => s.ensure(consumer, 0, 0)); return () => untrack(() => s.ensure(consumer, 0, 0)); });

  const states = $derived((bind.states ?? {}) as { empty?: State; loading?: State; error?: State; refine_query?: State });
  const errorMessage: State = $derived(states.error ?? { title: "This list could not be loaded." });
  const refineMessage: State = $derived(states.refine_query ?? { title: "Refine your search." });
  const skeletons = [70, 52, 64, 44, 58];
  /** The system row height (a token), so the scroll extent is right before any row is measured. */
  const rowH = cssPx("--row-h", 44);
</script>

<div class="list" bind:this={root} onkeydowncapture={onkey} data-bind={bind.id} data-state={store.state} data-count={store.length} data-hidden={store.hidden || undefined}>
  {#if store.state === "loading" || store.hidden}
    <div class="sk" role="status" aria-label={states.loading?.title ?? "Loading"}>
      {#each skeletons as w, k (k)}<SkeletonRow width={w} icon={template.icon !== undefined} />{/each}
    </div>
    {#if states.loading}<StateMessage message={states.loading} />{/if}
  {:else if store.state === "empty"}
    {#if states.empty}<StateMessage message={states.empty} />{/if}
  {:else if store.state === "error"}
    <StateMessage message={errorMessage} tone="bad" live="alert" />
  {:else}
    {#if store.state === "refine_query"}<div class="refine"><StateMessage message={refineMessage} /></div>{/if}
    <div class="rows" class:inside>
      <VirtualList bind:this={vl}
        count={store.length} estimate={rowH} startAtBottom={false} {inside} {label} rowRole="listitem"
        onrange={(first, end) => store.ensure(consumer, first, end)}
        nav={{
          pageKeys: "scroll",
          id: (i) => store.row(i)?.id ?? "", indexOf: (id) => store.indexOf(id), selected: () => (selection.drawn ? store.indexOf(selection.drawn) : -1),
          onmove: (i) => { const r = store.row(i); if (r) selection.paint(r.id); },
          open: (i) => { const r = store.row(i); if (r) choose(r.id, i, false); },
          onjump: (key) => { if (key === "End") { store.jump("end"); wantEnd = store.walkGoal !== null; } },
        }}
      >
        {#snippet row(i)}
          {@const r = store.row(i)}
          {#if r}{#if multiple}<label class="tick"><input type="checkbox" checked={selection.ticked.includes(r.id)} onchange={() => selection.toggle(r.id)} aria-label="Select {r.id}" /></label>{/if}<RowActions {template} row={r} {store} {kindIcon} selected={r.id === selection.drawn} onclick={() => choose(r.id, i, true)} />
          {:else}<SkeletonRow icon={template.icon !== undefined} width={40 + ((i * 17) % 40)} />{/if}
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

<style>
  .list { position: relative; display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .rows { position: relative; flex: 1; min-height: 0; }
  .rows.inside { flex: none; }
  .refine { flex: none; }
  .tick { position: absolute; inset-inline-start: 4px; z-index: 1; display: grid; place-items: center; min-width: 28px; min-height: var(--row-h); }
</style>
