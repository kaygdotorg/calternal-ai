/**
 * Opening in a NEW window, in one place for every strip app, section heading, row, card, cell or event, whatever component draws it:
 *  - ⌘ or Ctrl held on a click, ⌘/Ctrl+Enter on a focused one, or the middle button (`watchNewWindow`);
 *  - "Open in new window" in its context menu (shell/OpenMenu.svelte, right-click or Shift+F10).
 * The gesture is taken before the component sees the click, so no list or page needs to know. What an element opens is read from its data attributes (`target`); a new way of drawing an item only has to carry one of them.
 */
import { wins } from "../windows.svelte";
import { ITEMS } from "../data/collections";

export type Target = { kind: "app" | "item" | "coll"; id: string; el: HTMLElement };
/** The attributes (dataset names) whose value is an item's id. */
const ITEM_KEYS = ["item", "row", "task", "id", "note", "mailRow", "mailCard", "mailCell", "photo", "file", "event", "eventCell", "eventCard"];
const SELECTOR = ITEM_KEYS.map((k) => `[data-${k.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase())}]`).join(", ");

/** What the element (or an ancestor) opens: a strip app, a section heading's collection, or an item. Null for anything else. */
export function target(el: Element): Target | null {
  const app = el.closest<HTMLElement>("[data-app]"); if (app) return { kind: "app", id: app.dataset.app!, el: app };
  const head = el.closest<HTMLElement>("[data-sidebar-body] .tog"), sec = head?.closest<HTMLElement>("[data-cid]"); if (head && sec) return { kind: "coll", id: sec.dataset.cid!, el: head };
  for (let n = el.closest<HTMLElement>(SELECTOR); n; n = n.parentElement?.closest<HTMLElement>(SELECTOR) ?? null) {
    for (const k of ITEM_KEYS) { const id = n.dataset[k]; if (id && ITEMS[id]) return { kind: "item", id, el: n }; }
  }
  return null;
}

/** Opens the target in a new window beside the focused one, even if it is already open. `onApp` also points the sidebar at an app opened this way. */
export function openNew(t: Target, onApp: (id: string) => void) {
  if (t.kind === "app") onApp(t.id);
  wins.beside(t.kind === "app" ? "app" : t.kind === "coll" ? "collection" : "item", t.id, true);
}

/** Watches the new-window gestures (desktop). Returns the way to stop. */
export function watchNewWindow(onApp: (id: string) => void, enabled: () => boolean): () => void {
  const take = (e: Event) => {
    if (!enabled()) return; const t = target(e.target as Element); if (!t) return;
    e.preventDefault(); e.stopImmediatePropagation(); openNew(t, onApp);
  };
  const click = (e: MouseEvent) => { if (e.metaKey || e.ctrlKey) take(e); };
  const key = (e: KeyboardEvent) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && (e.target as Element).closest?.("button, a")) take(e); };
  const aux = (e: MouseEvent) => { if (e.button === 1) take(e); };
  // the middle button's own default (autoscroll) is stopped on anything that opens
  const down = (e: MouseEvent) => { if (e.button === 1 && enabled() && target(e.target as Element)) e.preventDefault(); };
  addEventListener("click", click, true); addEventListener("keydown", key, true); addEventListener("auxclick", aux, true); addEventListener("mousedown", down, true);
  return () => { removeEventListener("click", click, true); removeEventListener("keydown", key, true); removeEventListener("auxclick", aux, true); removeEventListener("mousedown", down, true); };
}
