/**
 * The seam between the mockup's shell and an app. An app lives in `src/apps/<app>/` and is added to the mockup with one registration line in `src/apps/index.ts`. Registering does five things, all before the shell starts:
 *  1. puts the app in the card's Apps tab (`id`, `name`, `icon`), replacing the generic stand-in of the same id;
 *  2. declares its fixture data as collections (`collections`, built with `mk`, `seq`, `pick`, `def` from `data/collections.ts`);
 *  3. says which of them are its sidebar, in order (`sidebar`: collection ids; each is drawn small, and each can be opened big, rearranged, hidden, renamed by the person like any section) and an optional fixed action row (`action`: "New note");
 *  4. supplies the page its window shows (`page`), and optionally a look for a kind (`looks`: how an item of that kind is drawn in rows, strips, cards and cells) and, if the generic look of its kinds is not enough, a page for an item of a kind (`itemPage`);
 *  5. supplies the app's right panel (`panel`) for its windows; without one the shared panel (properties, participants, activity, assistant) is shown.
 * An app uses the shared components in `src/shell` (Item, Mark, PageThumb, Av, AppIcon) and `@calternal/ui`, the tokens (`--text-*`, `--s-*`, `--r-*`, `--dur-*`, `--ease-*`) and nothing else for size, colour or motion; no literal px sizes for text.
 */
import type { Component } from "svelte";
import { APPS, VISITS } from "../data/apps";
import { ACTIONS, COLLS, DEFAULT_SIDEBARS, KIND_APP, type Coll, type Item, type Kind } from "../data/collections";
import type { Win } from "../windows.svelte";
import { arr } from "../arrange.svelte";

/** The props a page component receives: the window it is drawn in, and `onopen` to open an item (it opens in place, or in a window beside, by the canvas's own rule). */
export interface PageProps { win: Win; onopen: (item: Item) => void; oncollection: (cid: string) => void }
/** `face` is only the picture of an item, filling the frame its place gives it (a gallery tile): a look that draws pictures (files, photos) supplies it; for every other kind the shell draws the face. */
/** The props of a look: what `Item.svelte` passes to it. */
export type LookMode = "row" | "strip" | "card" | "cell" | "face";
/** A look is one component that draws every shape (it gets `mode`), or one component per shape (a missing shape keeps the shared look). */
export type Look = Component<LookProps> | Partial<Record<LookMode, Component<LookProps>>>;
export interface LookProps { item: Item; mode: LookMode; big?: boolean; /** In a big row: the comfortable density, with the secondary text on a second line. */ two?: boolean; selected?: boolean; onopen?: (i: Item) => void }
export interface MockupApp {
  id: string; name: string; /** A name from packages/ui's icon set (or one `shell/AppIcon.svelte` draws). */ icon: string;
  /** The item kinds this app opens in place (a Mail window opens a mail, not a window beside). */ kinds?: Kind[];
  /** Days-ago of recent opens, for the card's frecency order. */ visits?: number[];
  collections: Coll[]; sidebar: string[]; action?: string;
  /** A look for a kind, drawn by `shell/Item.svelte` in place of its own branch for that kind (a file's type tile, a photo's thumbnail). It receives the item and the shape wanted (`row`, `strip`, `card`, `cell`). */
  looks?: Partial<Record<Kind, Look>>;
  /** The title of the window in the canvas's title pill for this app's window (e.g. the open folder or file); empty or undefined keeps the app's name. */
  /** Called when the person presses the sidebar's fixed action row ("Write", "New event"): open a window, a compose sheet, anything. */
  onAction?: () => void;
  title?: (win: Win) => string | undefined;
  page?: Component<PageProps>; itemPage?: Partial<Record<Kind, Component<{ item: Item; win?: Win }>>>; panel?: Component<{ win: Win | undefined; item?: Item }>;
}
export const pages: Record<string, Component<PageProps>> = {};
export const itemPages: Partial<Record<Kind, Component<{ item: Item; win?: Win }>>> = {};
export const actionHandlers: Record<string, () => void> = {};
export const looks: Partial<Record<Kind, Look>> = {};
export const titles: Record<string, (win: Win) => string | undefined> = {};
export const panels: Record<string, Component<{ win: Win | undefined; item?: Item }>> = {};

/** Adds an app to the mockup. Call it from `src/apps/index.ts`, once per app. */
export function registerApp(app: MockupApp): void {
  const at = APPS.findIndex((a) => a.id === app.id), def = { id: app.id, name: app.name, icon: app.icon };
  if (at >= 0) APPS[at] = def; else APPS.push(def);
  for (const c of app.collections) COLLS[c.id] = c;
  const old = DEFAULT_SIDEBARS[app.id]; DEFAULT_SIDEBARS[app.id] = app.sidebar; arr.seed(app.id, app.sidebar, old); // works whether the arrangement exists yet or not
  if (app.action) ACTIONS[app.id] = app.action; else delete ACTIONS[app.id];
  if (app.visits) VISITS[app.id] = app.visits;
  for (const k of app.kinds ?? []) KIND_APP[k] = app.id;
  for (const [k, v] of Object.entries(app.looks ?? {})) looks[k as Kind] = v;
  if (app.title) titles[app.id] = app.title;
  if (app.onAction) actionHandlers[app.id] = app.onAction;
  if (app.page) pages[app.id] = app.page;
  if (app.panel) panels[app.id] = app.panel;
  for (const [k, v] of Object.entries(app.itemPage ?? {})) itemPages[k as Kind] = v;
}
