/**
 * The description as the shell's schema has it (contract/chrome-v1.schema.json on lane/ios-shell): icons are plain catalogue names, a badge's `value` is a string (and the icon's name for kind `icon`), a selector option's `badge` is a
 * string, the appearance is `{wallpaper, blur, dim, tint, uiSize, image?}`. This page builds the richer description of `types.ts` from its stores; this file is the one place that turns it into what the schema allows, so a change of the
 * contract is a change here. Fields the schema does not have are not sent.
 */
import type { ChromeBadge, ChromeDescription, ChromeItem, ChromeRow, ChromeSection } from "./types";

const item = (i: ChromeItem): Record<string, unknown> => ({
  id: i.id, title: i.title, ...(i.icon ? { icon: i.icon } : i.symbol ? { icon: { symbol: i.symbol } } : {}), ...(i.role ? { role: i.role } : {}), ...(i.disabled ? { disabled: true } : {}), ...(i.checked ? { checked: true } : {}),
  ...(i.sub ? { sub: i.sub } : {}), ...(i.input ? { input: i.input } : {}), ...(i.menu ? { menu: i.menu.map(item) } : {}),
});
const badge = (b: ChromeBadge): Record<string, unknown> => ({ kind: b.kind, ...(b.kind === "icon" ? { value: b.icon ?? b.symbol ?? "" } : b.value !== undefined ? { value: String(b.value) } : {}), ...(b.tone ? { tone: b.tone } : {}) });
const row = (r: ChromeRow): Record<string, unknown> => ({
  id: r.id, title: r.title, ...(r.icon ? { icon: r.icon } : r.symbol ? { icon: { symbol: r.symbol } } : {}), ...(r.trailing?.length ? { trailing: r.trailing.map(badge) } : {}),
  ...(r.selected ? { selected: true } : {}), ...(r.label ? { label: r.label } : {}), ...(r.hint ? { hint: r.hint } : {}),
  ...(r.leadingActions?.length ? { leadingActions: r.leadingActions.map(item) } : {}), ...(r.trailingActions?.length ? { trailingActions: r.trailingActions.map(item) } : {}), ...(r.menu?.length ? { menu: r.menu.map(item) } : {}),
});
const section = (s: ChromeSection): Record<string, unknown> => ({
  id: s.id, ...(s.title ? { title: s.title } : {}), ...(s.collapsed ? { collapsed: true } : {}), ...(s.menu?.length ? { menu: s.menu.map(item) } : {}), ...(s.actions?.length ? { actions: s.actions.map(item) } : {}),
  rows: s.rows.map(row), ...(s.count !== undefined ? { count: s.count, offset: s.offset ?? 0 } : {}),
});

/** The description in the shell's schema form. */
export function toSchema(d: ChromeDescription): unknown {
  return {
    version: 1, revision: d.revision, scheme: d.scheme, ...(d.accent ? { accent: d.accent } : {}), ...(d.appearance ? { appearance: d.appearance } : {}),
    header: { title: d.header.title, ...(d.header.leading?.length ? { leading: d.header.leading.map(item) } : {}), ...(d.header.trailing?.length ? { trailing: d.header.trailing.map(item) } : {}) },
    sidebar: {
      open: d.sidebar.open, title: d.sidebar.title, refresh: true,
      selectors: d.sidebar.selectors.map((s) => ({ id: s.id, selected: s.selected, options: s.options.map((o) => ({ id: o.id, title: o.title, ...(o.badge?.value !== undefined ? { badge: String(o.badge.value) } : {}) })) })),
      sections: d.sidebar.sections.map(section),
      apps: d.sidebar.apps.map((a) => ({ id: a.id, title: a.title, icon: a.icon ?? (a.symbol ? { symbol: a.symbol } : "apps"), selected: a.selected })),
      account: { id: d.sidebar.account.id, initial: d.sidebar.account.initial, title: d.sidebar.account.title, ...(d.sidebar.account.menu?.length ? { menu: d.sidebar.account.menu.map(item) } : {}) },
      search: d.sidebar.search,
    },
  };
}
