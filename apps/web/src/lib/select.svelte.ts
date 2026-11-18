// Selecting several rows at once (#470, #212). One store for every list: the rows know their own
// id (`data-select-id`), the list says what is in it and in what order, and the actions that work
// on the selection come from the bulk registry (bulk.ts). Nothing here is about notes.
//
// Ported unchanged from the existing app (rows/select.svelte.ts). Ways in here: "Select" in a row's menu, ⌘/Ctrl-click or ⇧-click. Taps then add or remove. Ways out: Done, Esc, or the bulk action that used the
// rows up. (The two-finger swipe down of the existing app is not ported yet.)
export type SelectSource = {
  /** Every id in the list, in the order shown. */
  order: () => string[];
};

// The list on screen says what it holds; a row's menu or the page's ⋯ can then start selecting
// without knowing which list it is in.
const provided = new Map<string, SelectSource>();
export function provideList(kind: string, src: SelectSource): () => void {
  provided.set(kind, src);
  return () => {
    if (provided.get(kind) === src) provided.delete(kind);
  };
}

export const selection = $state({
  on: false,
  kind: "",
  // Replaced whole on every change (never edited in place): one signal for the set, so Select all on
  // 10,000 notes is one assignment, not 10,000 reactive writes. Rows read `ids.has(id)`; only the
  // few that are drawn re-read it.
  ids: new Set<string>() as ReadonlySet<string>,
  /** The last row picked, where ⇧-click starts its range. */
  anchor: null as string | null,
  /** The last change came from a key: nothing about it animates. */
  quiet: false,
});

let source: SelectSource | null = null;

const sync = () => {
  const root = document.documentElement;
  if (selection.on) root.dataset.selecting = "";
  else delete root.dataset.selecting;
  if (selection.quiet) root.dataset.quietSelect = "";
  else delete root.dataset.quietSelect;
};

/** Starts selecting in a list. `first` is picked straight away. */
export function beginSelect(kind: string, first?: string, quiet = false) {
  if (selection.on && selection.kind !== kind) endSelect();
  source = provided.get(kind) ?? null;
  selection.on = true;
  selection.kind = kind;
  selection.quiet = quiet;
  selection.ids = new Set(first ? [first] : []);
  selection.anchor = first ?? null;
  sync();
}

export function endSelect() {
  selection.on = false;
  selection.kind = "";
  selection.ids = new Set();
  selection.anchor = null;
  source = null;
  sync();
}

export const isPicked = (id: string) => selection.on && selection.ids.has(id);
export const selecting = (kind: string) => selection.on && selection.kind === kind;

export function toggleId(id: string, quiet = false) {
  selection.quiet = quiet;
  const next = new Set(selection.ids);
  if (next.delete(id)) selection.ids = next;
  else {
    next.add(id);
    selection.ids = next;
    selection.anchor = id;
  }
  sync();
}

/** ⇧-click: everything from the anchor to this row. */
export function pickRange(id: string, quiet = false) {
  const order = source?.order() ?? [];
  const a = selection.anchor ? order.indexOf(selection.anchor) : -1;
  const b = order.indexOf(id);
  selection.quiet = quiet;
  if (a < 0 || b < 0) return toggleId(id, quiet);
  const next = new Set(selection.ids);
  for (const x of order.slice(Math.min(a, b), Math.max(a, b) + 1)) next.add(x);
  selection.ids = next;
  sync();
}

export function selectAll(quiet = false) {
  selection.quiet = quiet;
  selection.ids = new Set(source?.order() ?? []);
  sync();
}

export function setPicked(ids: Iterable<string>, quiet = false) {
  selection.quiet = quiet;
  selection.ids = new Set(ids);
  sync();
}

export const allPicked = () => {
  const order = source?.order() ?? [];
  return order.length > 0 && order.every((id) => selection.ids.has(id));
};
export const orderNow = () => source?.order() ?? [];

/** A row went away (archived, deleted): it can't stay picked. */
export function forgetPicked(ids: string[]) {
  const next = new Set(selection.ids);
  for (const id of ids) next.delete(id);
  selection.ids = next;
}
