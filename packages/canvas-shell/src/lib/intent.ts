/**
 * Opening in a NEW window, in one place for every strip app, section heading, row, card, cell or event:
 *  - Cmd or Ctrl held on a click, Cmd/Ctrl+Enter on a focused one, or the middle button (`watchNewWindow`);
 *  - "Open in new window" in its context menu.
 */
import type { Windows } from "../windows.svelte";

export type Target = { kind: "app" | "item" | "coll"; id: string; el: HTMLElement };

/** The attributes (dataset names) whose value is an item's id. */
const ITEM_KEYS = ["item", "row", "task", "id", "note", "mailRow", "mailCard", "mailCell", "photo", "file", "event", "eventCell", "eventCard"];
const SELECTOR = ITEM_KEYS.map((k) => `[data-${k.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase())}]`).join(", ");

/** What the element (or an ancestor) opens: a strip app, a section heading's collection, or an item. Null for anything else. */
export function target(el: Element, hasItem: (id: string) => boolean = () => true): Target | null {
  const app = el.closest<HTMLElement>("[data-app]");
  if (app) return { kind: "app", id: app.dataset.app!, el: app };
  const head = el.closest<HTMLElement>("[data-sidebar-body] .tog");
  const sec = head?.closest<HTMLElement>("[data-cid]");
  if (head && sec) return { kind: "coll", id: sec.dataset.cid!, el: head };
  for (let n = el.closest<HTMLElement>(SELECTOR); n; n = n.parentElement?.closest<HTMLElement>(SELECTOR) ?? null) {
    for (const k of ITEM_KEYS) {
      const id = n.dataset[k];
      if (id && hasItem(id)) return { kind: "item", id, el: n };
    }
  }
  return null;
}

/** Opens the target in a new window beside the focused one, even if it is already open. */
export function openNew(wins: Windows, t: Target, onApp: (id: string) => void) {
  if (t.kind === "app") onApp(t.id);
  wins.beside(t.kind === "app" ? "app" : t.kind === "coll" ? "collection" : "item", t.id, true);
}

/** Watches the new-window gestures (desktop). Returns the way to stop. */
export function watchNewWindow(
  wins: Windows,
  onApp: (id: string) => void,
  enabled: () => boolean,
  hasItem: (id: string) => boolean = () => true,
): () => void {
  const take = (e: Event) => {
    if (!enabled()) return;
    const t = target(e.target as Element, hasItem);
    if (!t) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    openNew(wins, t, onApp);
  };
  const click = (e: MouseEvent) => { if (e.metaKey || e.ctrlKey) take(e); };
  const key = (e: KeyboardEvent) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && (e.target as Element).closest?.("button, a")) take(e);
  };
  const aux = (e: MouseEvent) => { if (e.button === 1) take(e); };
  const down = (e: MouseEvent) => {
    if (e.button === 1 && enabled() && target(e.target as Element, hasItem)) e.preventDefault();
  };
  addEventListener("click", click, true);
  addEventListener("keydown", key, true);
  addEventListener("auxclick", aux, true);
  addEventListener("mousedown", down, true);
  return () => {
    removeEventListener("click", click, true);
    removeEventListener("keydown", key, true);
    removeEventListener("auxclick", aux, true);
    removeEventListener("mousedown", down, true);
  };
}
