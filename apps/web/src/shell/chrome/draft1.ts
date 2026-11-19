/**
 * The first draft of the shell's contract (lane/ios-shell 9b033153) is narrower than what `types.ts` describes: its decoder refuses fields it does not know (a selector option's `badge`, a row's `trailing` list, an
 * `icon`, `accent`, section `actions`, an account `menu`, an item's `checked`/`sub`/`input`). Until the shell takes the agreed fields (its next revision), the publisher sends this reduced form: the same description with
 * each extra field folded into what the draft can say (a row's first trailing mark becomes its one `badge`, an icon becomes its SF Symbol), and everything else dropped. When the revision lands, `CONTRACT` in
 * `native-chrome.svelte.ts` is switched and this file is deleted.
 */
import type { ChromeBadge, ChromeDescription, ChromeItem, ChromeRow, ChromeSection } from "./types";

interface DraftItem { id: string; title: string; symbol?: string; role?: "destructive"; disabled?: boolean; menu?: DraftItem[] }
interface DraftRow { id: string; title: string; detail?: string; symbol?: string; badge?: { kind: "dot" | "count" | "text"; value?: number | string }; selected?: boolean; leadingActions?: DraftItem[]; trailingActions?: DraftItem[]; menu?: DraftItem[] }

/** SF Symbols for the page's icon names, for the first draft's decoder (the shell's own icon set takes over with the next revision). */
const SF: Record<string, string> = { threads: "bubble.left", notes: "doc.text", settings: "gearshape", plus: "plus", clock: "clock", wrench: "wrench", edit: "pencil", folder: "folder", link: "link", archive: "archivebox", trash: "trash", share: "square.and.arrow.up", more: "ellipsis", user: "person.crop.circle", signout: "rectangle.portrait.and.arrow.right", claude: "asterisk", codex: "chevron.left.forwardslash.chevron.right", question: "questionmark.circle", shield: "checkmark.shield", key: "key", lock: "lock", warning: "exclamationmark.triangle", bell: "bell", stop: "stop.circle", pulse: "waveform.path.ecg", "stop-round": "stop.circle", spark: "sparkles", image: "photo", list: "list.bullet", apps: "square.grid.2x2", file: "doc", terminal: "terminal", info: "info.circle", shield2: "shield" };
const sf = (i: { icon?: string; symbol?: string }): { symbol?: string } => { const v = i.symbol ?? (i.icon ? SF[i.icon] : undefined); return v ? { symbol: v } : {}; };
const item = (i: ChromeItem): DraftItem => ({ id: i.id, title: i.title, ...sf(i), ...(i.role ? { role: i.role } : {}), ...(i.disabled ? { disabled: true } : {}), ...(i.menu ? { menu: i.menu.map(item) } : {}) });
const badge = (b: ChromeBadge | undefined): DraftRow["badge"] => (!b ? undefined : b.kind === "count" ? { kind: "count", value: b.value } : b.kind === "text" ? { kind: "text", value: b.value } : b.tone === "bad" ? { kind: "text", value: "!" } : { kind: "dot" });

const row = (r: ChromeRow): DraftRow => ({
  id: r.id, title: r.title, ...(r.detail ? { detail: r.detail } : {}),
  ...sf(r),
  ...(r.trailing?.[0] ? { badge: badge(r.trailing[0]) } : {}), ...(r.selected ? { selected: true } : {}),
  ...(r.leadingActions?.length ? { leadingActions: r.leadingActions.map(item) } : {}), ...(r.trailingActions?.length ? { trailingActions: r.trailingActions.map(item) } : {}), ...(r.menu?.length ? { menu: r.menu.map(item) } : {}),
});
const section = (s: ChromeSection) => ({ id: s.id, ...(s.title ? { title: s.title } : {}), ...(s.collapsed ? { collapsed: true } : {}), ...(s.menu?.length ? { menu: s.menu.map(item) } : {}), rows: s.rows.map(row) });

/** The description in the shape the first draft of the contract can decode. */
export function toDraft1(d: ChromeDescription): unknown {
  return {
    version: 1, revision: d.revision, scheme: d.scheme,
    header: { title: d.header.title, ...(d.header.leading?.length ? { leading: d.header.leading.map(item) } : {}), ...(d.header.trailing?.length ? { trailing: d.header.trailing.map(item) } : {}) },
    sidebar: {
      open: d.sidebar.open, title: d.sidebar.title,
      selectors: d.sidebar.selectors.map((s) => ({ id: s.id, selected: s.selected, options: s.options.map((o) => ({ id: o.id, title: o.title })) })),
      sections: d.sidebar.sections.map(section),
      apps: d.sidebar.apps.map((a) => ({ id: a.id, title: a.title, ...sf(a), selected: a.selected })),
      account: { id: d.sidebar.account.id, initial: d.sidebar.account.initial, title: d.sidebar.account.title },
      search: d.sidebar.search,
    },
  };
}
