<script lang="ts" module>
  export interface BoardColumn<T> {
    id: string;
    label: string;
    items: T[];
  }
</script>

<script lang="ts" generics="T extends { id: string }">
  /**
   * Shared board (kanban) layout from `apps/mockup/src/canvas/views/BoardView.svelte`.
   * One column per lane with its name and count, pointer + touch drag-to-move with ghost
   * and rise motion (`--dur-base`, `--ease-out`), independent column scroll, and horizontal
   * phone scroll. Used by both `apps/mockup` and `apps/web`.
   */
  import type { Snippet } from "svelte";
  import { tokenEase } from "@calternal/ui";
  import { ms } from "../motion";

  let {
    columns,
    emptyText = "No items",
    canDrag = true,
    onmove,
    renderCard,
  }: {
    columns: BoardColumn<T>[];
    emptyText?: string;
    canDrag?: boolean;
    onmove?: (item: T, toColumnId: string) => void;
    renderCard: Snippet<[T, boolean]>;
  } = $props();

  let drag = $state<{ item: T; fromCol: string; x: number; y: number; over: string | null } | null>(null);
  let pend: { item: T; fromCol: string; x: number; y: number; pid: number; t: number } | null = null;
  let quiet = $state(false);
  let cols: HTMLElement[] = [];

  function down(e: PointerEvent, item: T, fromCol: string) {
    if (!canDrag || !onmove || e.button !== 0 || pend) return;
    pend = { item, fromCol, x: e.clientX, y: e.clientY, pid: e.pointerId, t: 0 };
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
    addEventListener("pointercancel", stop);
    addEventListener("keydown", key);
    if (e.pointerType !== "mouse") pend.t = window.setTimeout(() => pend && start(pend.x, pend.y), 450);
  }

  function start(x: number, y: number) {
    if (!pend) return;
    clearTimeout(pend.t);
    drag = { item: pend.item, fromCol: pend.fromCol, x, y, over: null };
    addEventListener("touchmove", ns, { passive: false });
    document.documentElement.style.userSelect = "none";
    navigator.vibrate?.(8);
  }

  const ns = (e: TouchEvent) => {
    if (drag) e.preventDefault();
  };

  function move(e: PointerEvent) {
    if (!pend || e.pointerId !== pend.pid) return;
    if (!drag) {
      if (Math.hypot(e.clientX - pend.x, e.clientY - pend.y) < (e.pointerType === "mouse" ? 5 : 8)) return;
      if (e.pointerType !== "mouse") return stop();
      start(e.clientX, e.clientY);
    }
    const d = drag!;
    d.x = e.clientX;
    d.y = e.clientY;
    d.over =
      cols.find((c) => {
        if (!c) return false;
        const b = c.getBoundingClientRect();
        return e.clientX >= b.left && e.clientX <= b.right && e.clientY >= b.top && e.clientY <= b.bottom;
      })?.dataset.col ?? null;
  }

  function up(e: PointerEvent) {
    if (!pend || e.pointerId !== pend.pid) return;
    const d = drag;
    stop();
    if (d) {
      quiet = true;
      setTimeout(() => (quiet = false), 0);
      if (d.over && d.over !== d.fromCol) onmove?.(d.item, d.over);
    }
  }

  const key = (e: KeyboardEvent) => e.key === "Escape" && stop();

  function stop() {
    removeEventListener("pointermove", move);
    removeEventListener("pointerup", up);
    removeEventListener("pointercancel", stop);
    removeEventListener("keydown", key);
    removeEventListener("touchmove", ns);
    if (pend) clearTimeout(pend.t);
    pend = null;
    drag = null;
    document.documentElement.style.userSelect = "";
  }

  const riseCard = (n: HTMLElement) => {
    n.animate(
      [{ opacity: 0, translate: "0 8px" }, { opacity: 1, translate: "0 0" }],
      { duration: ms("--dur-base", 220), easing: tokenEase("--ease-out", "cubic-bezier(0.23, 1, 0.32, 1)") },
    );
  };
</script>

<div class="board" data-board style:--lanes={columns.length}>
  {#each columns as col, i (col.id)}
    <section
      class="col"
      class:over={drag?.over === col.id}
      data-col={col.id}
      bind:this={cols[i]}
      aria-label={`${col.label}, ${col.items.length} items`}
    >
      <header>
        <strong>{col.label}</strong>
        <span class="n">{col.items.length}</span>
      </header>
      <div class="cards">
        {#each col.items as item (item.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="cw"
            class:held={drag?.item.id === item.id}
            onpointerdown={(e) => down(e, item, col.id)}
            use:riseCard
          >
            {@render renderCard(item, quiet)}
          </div>
        {/each}
        {#if !col.items.length}<p class="em">{emptyText}</p>{/if}
      </div>
    </section>
  {/each}
  {#if drag}
    <div class="ghost" style:left="{drag.x - 90}px" style:top="{drag.y - 24}px">
      {@render renderCard(drag.item, true)}
    </div>
  {/if}
</div>

<style>
  .board { display: grid; grid-template-columns: repeat(var(--lanes, 3), minmax(13rem, 1fr)); gap: var(--s-5); height: 100%; min-height: 0; align-items: stretch; overflow-x: auto; }
  .col { display: flex; flex-direction: column; min-height: 0; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); transition: background-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out); }
  .col.over { background: var(--accent-soft); box-shadow: inset 0 0 0 2px var(--accent); }
  header { flex: none; display: flex; align-items: center; gap: var(--s-4); padding: var(--s-5) var(--s-5) var(--s-3); font-size: var(--text-ui); }
  header strong { font-weight: var(--strong-weight); }
  .n { min-width: 20px; padding: 0 var(--s-3); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.1); color: var(--ink-2); font-size: var(--text-label); font-variant-numeric: tabular-nums; text-align: center; }
  .cards { flex: 1; min-height: 0; overflow-y: auto; display: grid; align-content: start; gap: var(--s-3); padding: var(--s-2) var(--s-3) var(--s-4); scrollbar-width: none; }
  .cw { touch-action: pan-y; }
  .cw.held { opacity: 0.35; }
  .em { margin: var(--s-4); color: var(--ink-3); font-size: var(--text-label); }
  .ghost { position: fixed; z-index: 95; width: 180px; pointer-events: none; rotate: 2deg; box-shadow: 0 12px 32px rgb(0 0 0 / 0.3); border-radius: var(--r-menu); }
  @media (max-width: 719px) { .board { grid-template-columns: repeat(var(--lanes, 3), 15rem); overflow-x: auto; } }
</style>
