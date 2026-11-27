/**
 * The mockup's own tweak panel state (mockup-only, never part of an app): the open points of the shell design as switches, so the owner can flip an assumption while looking. Values persist in localStorage and can be
 * forced by the address (`?wm=caveat&appname=cursive&scheme=dark&app=notes&coll=lisbon&phone=1&mac=1&tweaks=1&sheet=search`), which is how the captures are made.
 * It also holds the person's arrangement of the app switcher (their own order and pins), which is the one piece of real state here: nothing else is saved.
 */
export type Wordmark = "dancing" | "caveat" | "satisfy";
export interface Tweaks {
  /** The cursive of the wordmark (the first is the web app's own brand face). */
  wordmark: Wordmark;
  /** The face of the name the wordmark morphs into: the interface face, or the cursive again. */
  appName: "ui" | "cursive";
  scheme: "system" | "light" | "dark";
  /** The big list's density: one line per row (the default), or comfortable with a second line. A window can override it from its view menu. */
  density: "compact" | "comfortable";
  /** What the top line names: the sidebar, or (smart) the collection last opened from the card when no app was chosen after it. */
  topLine: "smart" | "sidebar";
  /** TASTE options (the owner's call; each default is the current behaviour, each alternative is the independent review's recommendation). T1: the open item's name appears in the pill and the page heading only; the right panel names the kind. */
  nameOnce: boolean;
  /** T5: a selected row is tinted a neutral ink grey (the accent stays on its mark) instead of the wallpaper's accent. */
  selTint: "accent" | "neutral";
  /** T7: gallery captions left-aligned or centred. */
  caps: "left" | "center";
  /** T6: the Collections tab marks each collection with a colour dot, or with the glyph of what it mostly holds (like the Apps tab). */
  collMarks: "dots" | "icons";
  /** Your role in the space (mockup only: decides whether "Make this the space's sidebar" is offered). */
  role: "Owner" | "Manager" | "Editor" | "Reader";
  /** A window's surface: frosted (a translucent card over the blurred wallpaper) or opaque. One switch (`data-winbg` on the root drives `--win-a`), so it can become a setting. */
  winBg: "frosted" | "opaque";
  /** The app switcher at the canvas's bottom centre: always shown, or hidden until the pointer reaches the bottom of the viewport (or the keyboard asks). A setting under Settings, Appearance. */
  switcherHide: boolean;
  /** What a switcher item shows: `auto` (text on a desktop, icons on a phone), text, icons, or both. */
  switcherLabels: "auto" | "text" | "icons" | "both";
  /** How many apps the switcher shows at once (the strip scrolls for the rest): 3 by default (the owner's sketch). */
  switcherCount: 3 | 4 | 5;
  /** The width of a window that is alone on the canvas: 640, 800 (default), 960 or the whole canvas. */
  loneW: "640" | "800" | "960" | "fill";
  /** T4: the profile popover as it is, or denser glass with a search over the spaces. */
  profile: "plain" | "dense";
  /** Draws the desktop shell inside a Mac window: traffic lights, the sidebar button, the wordmark below them. */
  mac: boolean;
}
const DEFAULTS: Tweaks = { wordmark: "dancing", appName: "cursive", scheme: "system", mac: false, density: "compact", topLine: "smart", nameOnce: false, selTint: "accent", caps: "left", collMarks: "dots", role: "Owner", winBg: "opaque", switcherHide: false, switcherLabels: "auto", switcherCount: 3, loneW: "800", profile: "plain" };
const KEY = "calternal.mockup.tweaks", ARR = "calternal.mockup.arrangement";
const q = new URLSearchParams(typeof location === "undefined" ? "" : location.search);

function load(): Tweaks {
  let s: Partial<Tweaks> = {};
  try { s = JSON.parse(localStorage.getItem(KEY) ?? "{}"); } catch { /* defaults */ }
  const t = { ...DEFAULTS, ...s };
  if (q.has("wm")) t.wordmark = q.get("wm") as Wordmark;
  if (q.has("appname")) t.appName = q.get("appname") as Tweaks["appName"];
  if (q.has("scheme")) t.scheme = q.get("scheme") as Tweaks["scheme"];
  if (q.has("mac")) t.mac = q.get("mac") === "1";
  for (const [k, key] of [["nameonce", "nameOnce"], ["sel", "selTint"], ["caps", "caps"], ["marks", "collMarks"], ["role", "role"], ["winbg", "winBg"], ["switcherlabels", "switcherLabels"], ["lonew", "loneW"], ["profile", "profile"]] as const) if (q.has(k)) (t as unknown as Record<string, unknown>)[key] = key === "nameOnce" ? q.get(k) === "1" : q.get(k);
  if (q.has("hide")) t.switcherHide = q.get("hide") === "1";
  if (q.has("density")) t.density = q.get("density") as Tweaks["density"];
  if (q.has("topline")) t.topLine = q.get("topline") as Tweaks["topLine"];
  return t;
}
function loadArr(): { own: string[] | null; pinned: string[] } {
  if (q.has("fresh")) return { own: null, pinned: [] };
  try { const a = JSON.parse(localStorage.getItem(ARR) ?? "null"); if (a && Array.isArray(a.pinned)) return { own: Array.isArray(a.own) ? a.own : null, pinned: a.pinned }; } catch { /* none */ }
  return { own: null, pinned: [] };
}

/** The reactive tweaks, the card size the person chose, and their arrangement of the apps. */
class State {
  t = $state<Tweaks>(load());
  /** The person's own order of app ids once they have pinned or dragged one; null while the order is frecency. */
  own = $state<string[] | null>(loadArr().own);
  /** The pinned apps: first in the card, in their own order. */
  pinned = $state<string[]>(loadArr().pinned);
  /** Opens made in this visit, added to the fixture history for frecency (the card re-sorts when it is next shown, not under the pointer). */
  opens = $state<Record<string, number>>({});
  set<K extends keyof Tweaks>(k: K, v: Tweaks[K]) { this.t[k] = v; try { localStorage.setItem(KEY, JSON.stringify(this.t)); } catch { /* private mode */ } }
  private save() { try { localStorage.setItem(ARR, JSON.stringify({ own: this.own, pinned: this.pinned })); } catch { /* private mode */ } }
  /** The person set an order by dragging (or moving with the keyboard): from now on it is theirs, and frecency no longer decides. */
  setOrder(ids: string[]) { this.own = [...ids]; this.save(); }
  /** Pins or unpins an app; pinning makes the order the person's own as well. */
  togglePin(id: string, current: string[]) { this.pinned = this.pinned.includes(id) ? this.pinned.filter((x) => x !== id) : [...this.pinned, id]; this.own ??= [...current]; this.save(); }
  /** Back to frecency, as before anything was pinned or dragged. */
  resetOrder() { this.own = null; this.pinned = []; this.save(); }
  visit(id: string) { this.opens[id] = (this.opens[id] ?? 0) + 1; }
}
/** Whether the mockup's tweak panel is open (it is reached from the profile menu, or Alt+Shift+T). */
export const panel = $state({ open: false });
export const tweaks = new State();
/** The address's forced selection (for captures), if any. */
export const forced = { app: q.get("app"), coll: q.get("coll"), phone: q.get("phone") === "1", tweaks: q.get("tweaks") === "1", sheet: q.get("sheet"), plugin: q.get("plugin"), view: q.get("view") };
