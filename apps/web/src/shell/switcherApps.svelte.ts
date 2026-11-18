/**
 * The apps the switcher offers: the built-in ones in the person's order, then the apps that enabled plugins contribute (in the person's order too, appended when not named yet). A plugin's app is only DATA: an id, a
 * name (required: it is the icon's accessible name), an icon (a name from the host's icon set, or one SVG path string the host validates here: never an image, HTML or script) and the route prefix its address
 * lives under. The manifest field that carries it is the plugins lane's; `setPluginApps` is where its result goes. Fixtures: `?apps=N` shows exactly N apps (extra ones are generated), for checks at 1, 3, 4 and 20.
 */
import type { IconName } from "@calternal/ui";
import { ICONS } from "@calternal/ui";
import { appearance } from "../appearance/prefs.svelte";
import { APPS } from "../apps/registry";
import { serverMode } from "../server/client";
import { applyOrder } from "./appOrder";

/** One app in the switcher. */
export interface SwitcherApp {
  /** The kind in addresses (`/<kind>/…`). */
  kind: string;
  /** The app's name: the icon's accessible name and tooltip. Required. */
  name: string;
  /** A name from the host's icon set. */
  icon?: IconName;
  /** One validated SVG path (`d`), drawn on the 24 grid, for an app whose plugin brings its own icon. */
  iconPath?: string;
}

const PATH_OK = /^[MmLlHhVvCcSsQqTtAaZz0-9eE.,\s+-]+$/;
/** An SVG path string the host will draw: only path commands and numbers, at most 600 characters. Anything else is refused (no icon: the app gets its first letter in a circle). */
export const safePath = (d: unknown): d is string => typeof d === "string" && d.length > 0 && d.length <= 600 && PATH_OK.test(d);
/** The first letter of an app's name, for an app with no usable icon. */
export const letterOf = (name: string): string => [...name.trim()][0]?.toUpperCase() ?? "?";

class PluginApps {
  /** The apps contributed by enabled plugins. */
  list = $state<SwitcherApp[]>([]);
}
const plugin = new PluginApps();

/** Replaces the apps that plugins contribute (an app without a name, or one that repeats a kind, is dropped; an icon that is neither in the set nor a safe path is dropped, leaving the letter). */
export function setPluginApps(apps: SwitcherApp[]): void {
  const seen = new Set(APPS.map((a) => a.kind));
  plugin.list = apps.filter((a) => a.name?.trim() && a.kind && !seen.has(a.kind) && !!seen.add(a.kind)).map((a) => ({ kind: a.kind, name: a.name.trim(), icon: a.icon && a.icon in ICONS ? a.icon : undefined, iconPath: safePath(a.iconPath) ? a.iconPath : undefined }));
}

const fixtureCount = (): number | null => {
  if (serverMode() || typeof location === "undefined") return null;
  const n = Number(new URLSearchParams(location.search).get("apps"));
  return Number.isFinite(n) && n >= 1 ? Math.min(60, Math.floor(n)) : null;
};

/** The apps to show now, in the person's order. */
export function switcherApps(): SwitcherApp[] {
  const all: SwitcherApp[] = [...APPS.map((a) => ({ kind: a.kind, name: a.name, icon: a.icon })), ...plugin.list];
  const n = fixtureCount();
  const ordered = applyOrder(all, appearance.prefs.appOrder);
  if (n === null) return ordered;
  const extra: SwitcherApp[] = Array.from({ length: Math.max(0, n - ordered.length) }, (_, i) => ({ kind: `app-${i + 1}`, name: `App ${i + 1}`, icon: (["files", "table", "tasks", "image", "terminal", "code", "pulse", "bell"] as IconName[])[i % 8] }));
  return [...ordered, ...extra].slice(0, n);
}
