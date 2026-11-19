/**
 * The native chrome description: what the page publishes to the iPhone shell (`window.calternalNative.chrome(description)`) so the shell can draw the sidebar and the header natively, and the events it sends back
 * (`window` `calternal:chrome`). The contract is the shell lane's (docs/ios-shell.md on lane/ios-shell, "Native chrome"); this file is the page's copy of its shapes, with the fields agreed on 2026-11-19: `accent`, row
 * `icon` and several `trailing` badges, section `actions`, an account `menu` with `checked` and `sub`, selector option `badge`, the `refresh` event, an `input` prompt on an item, and `label`/`hint`. The shell knows nothing
 * about threads or notes: it draws rows, selectors, apps and buttons. When the contract changes, only this file and `describe` in `native-chrome.svelte.ts` change.
 */

/** An icon is a CalternalUI icon name (`threads`, `plus`, `settings`, `claude`, `codex`, ...: the page's own icon set, which the shell draws natively); `symbol` stays as the escape for an SF Symbol name. */
export type ChromeIconName = string;

/** A small mark at a row's end or on a selector option: a dot, a count, a word or a symbol, with a tone. */
export interface ChromeBadge { kind: "dot" | "count" | "text" | "icon"; value?: number | string; icon?: ChromeIconName; symbol?: string; tone?: "info" | "warn" | "bad" }

/** A button, a menu entry or a swipe action. `input` asks the shell for a line of text (a rename) and returns it as `text` in the event. */
export interface ChromeItem { id: string; title: string; icon?: ChromeIconName; symbol?: string; role?: "destructive"; disabled?: boolean; checked?: boolean; sub?: string; input?: { placeholder?: string; value?: string }; menu?: ChromeItem[] }

/** One row of a section. */
export interface ChromeRow {
  id: string; title: string; icon?: ChromeIconName; symbol?: string; trailing?: ChromeBadge[]; selected?: boolean;
  /** Spoken name and hint when the title alone is not enough. */
  label?: string; hint?: string;
  leadingActions?: ChromeItem[]; trailingActions?: ChromeItem[]; menu?: ChromeItem[];
}

/** A titled (or untitled: `title` null) group of rows that can fold. */
/** `count` and `offset` make `rows` a window of a longer list (rows are then one fixed height): the shell asks for the part it shows with a `window` event. */
export interface ChromeSection { id: string; title: string | null; collapsed?: boolean; menu?: ChromeItem[]; actions?: ChromeItem[]; rows: ChromeRow[]; count?: number; offset?: number }

/** A segmented choice above the lists (Inbox, All Threads). */
export interface ChromeSelector { id: string; selected: string; options: { id: string; title: string; badge?: ChromeBadge }[] }

/** The person's look, published so the shell can paint the wallpaper and the glass natively (contract/chrome-v1.schema.json): `wallpaper` is `auto`, a name the shell knows (`dune`, `aurora`, `ember`) or a library id (then `image` carries the picture once, as a PNG or JPEG data URL, and the shell blurs and solves the veils itself), or null for none. */
export interface ChromeAppearance { wallpaper: string | null; blur: boolean; dim: boolean; tint: boolean; uiSize: number; material: "wallpaper" | "translucent"; frost: number; image?: string }

/** The whole description. */
export interface ChromeDescription {
  version: 1; revision: number; scheme: "light" | "dark"; accent?: string;
  /** What the shell needs to paint the wallpaper and the glass natively (see `appearance.ts`). */
  appearance?: ChromeAppearance;
  header: { leading?: ChromeItem[]; title: string; trailing?: ChromeItem[] };
  sidebar: {
    open: boolean; title: string;
    selectors: ChromeSelector[]; sections: ChromeSection[];
    apps: { id: string; title: string; icon?: ChromeIconName; symbol?: string; selected: boolean }[];
    account: { id: string; initial: string; title: string; menu?: ChromeItem[] };
    search: { placeholder: string };
  };
}

/** What an app contributes: the parts of the description that depend on it (the header and the sidebar's selectors, sections and search); the shell fills in the rest. */
export interface ChromeBody { header: ChromeDescription["header"]; selectors: ChromeSelector[]; sections: ChromeSection[]; search: { placeholder: string } }

/** An event from the shell. */
export interface ChromeEvent { type: "row" | "action" | "menu" | "selector" | "app" | "header" | "account" | "search" | "section" | "sidebar" | "refresh" | "window"; id?: string; rowId?: string; option?: string; text?: string; open?: boolean; from?: number; to?: number; revision: number; sentAt: number }

/** An app's side of the contract: what it publishes and how it answers events it owns (`row`, `action`, `menu`, `selector`, `section`, `header`, `refresh`). */
export interface ChromeProvider {
  /** The body of the description for the current state of the app's stores (called inside an effect: it is re-read whenever they change). */
  describe(query: string): ChromeBody;
  /** Handles an event; returns true when it was one of this app's. */
  handle(ev: ChromeEvent): boolean;
}
