/**
 * The page's half of the native chrome experiment (docs/ios-shell.md on lane/ios-shell, "Native chrome"): behind a feature check on `window.calternalNative.chrome`, the page publishes a description of its navigation
 * chrome (header, sidebar sections, rows, selectors, apps, account, search) after every change of the stores it is built from, and answers the shell's `calternal:chrome` events. The shell replies `{native}`: when true the page
 * hides its own sidebar and header and pads the card (`data-native-chrome` on the root and `--chrome-top`, `--chrome-bottom` are the shell's); when false (the switch is off, or a browser) nothing changes.
 * One provider per app (`registerChrome`); the shell's own parts (the apps, the account menu, the scheme and the accent) are built here. Publications are coalesced to one per frame and each one's round trip is
 * measured (`window.__chromeStats`, and the bridge's log). In a browser nothing is installed.
 */
import { appearance } from "../../appearance/prefs.svelte";
import { navigate } from "../../lib/route.svelte";
import { accentOf } from "./accent";
import { appearanceOf } from "./appearance";
import { accountItems, pickAccountItem } from "../accountMenu";
import { self } from "../self.svelte";
import { switcherApps } from "../switcherApps.svelte";
import { toSchema } from "./encode";
import { countsOnly, selectionEchoOnly } from "./diff";
import type { ChromeBody, ChromeDescription, ChromeEvent, ChromeItem, ChromeProvider } from "./types";


interface Bridge { chrome?: (d: ChromeDescription | null) => Promise<{ native: boolean; reason?: string; supported?: number[] }>; log?: (s: string) => unknown; chromeVersions?: number[] }
const bridge = (): Bridge | undefined => (typeof window === "undefined" ? undefined : (window as unknown as { calternalNative?: Bridge }).calternalNative);

/** The version of the chrome contract this page publishes (`version` in the description; contract/chrome-v1.schema.json). */
export const PAGE_CONTRACT = 1;

/**
 * The compatibility rule (docs/ui-shell.md, "Native chrome: the page's side, and the compatibility rule"): the page and the shell negotiate the chrome contract over the bridge, and on ANY mismatch native chrome stays off and the
 * page draws its own, never a half-drawn shell (the phone's shell and the server's page are not always updated together). The shell exposes `calternalNative.chromeVersions` (the contract versions it can draw, today `[1]`). The page
 * publishes only when it has the call and `chromeVersions` is a non-empty list that includes the version the page encodes; a shell with no `chromeVersions` (an older one), an empty list or no common version is never called, so
 * the page's own chrome stays. The shell's answer to a publication is `{native}`: only `native: true` means it is drawn; `{native: false, reason}` (`unsupported-version` with `supported`, or `rejected` for a malformed description)
 * means the page draws its own, and it also clears whatever the shell had drawn.
 */
export function chromeCompatible(b: Pick<Bridge, "chrome" | "chromeVersions"> | undefined = bridge()): boolean {
  if (typeof b?.chrome !== "function") return false;
  const v = b.chromeVersions;
  return Array.isArray(v) && v.length > 0 && v.includes(PAGE_CONTRACT);
}

/** Whether the shell's answer to a publication means native chrome is on: only an explicit `native: true`; anything else (a refusal with a `reason`, a missing field, an error) is off. */
export const answerIsNative = (r: { native?: boolean } | null | undefined): boolean => !!r && r.native === true;

/** Whether the page can publish chrome at all (the call exists and the contract versions agree). */
export const chromeAvailable = (): boolean => chromeCompatible();

class NativeChrome {
  /** The shell drew the chrome for the last publication (it said `native: true`). */
  native = $state(false);
  /** Whether the person has the native sidebar open (the shell tells us when it changes). */
  open = $state(false);
  /** The text in the native search field. */
  query = $state("");
  /** Bumped when something the description reads outside the stores has arrived (a wallpaper picture): the publisher then builds it again. */
  tick = $state(0);
  /** The part of each long section the shell shows, by section id (`window` events): `[from, to]` row indexes. */
  windows = $state<Record<string, [number, number]>>({});
  /** Round trips of publications in milliseconds, newest last (at most 200), for the measurement. */
  lat: number[] = [];
  private revision = 0;
  private providers = new Map<string, ChromeProvider>();
  /** Registers the provider that publishes for the app of this kind. */
  register(kind: string, p: ChromeProvider): void { this.providers.set(kind, p); }
  providerFor(kind: string): ChromeProvider | undefined { return this.providers.get(kind); }
  nextRevision(): number { return ++this.revision; }
}
/** The native chrome's state (see the file's comment). */
export const nativeChrome = new NativeChrome();

/** Registers an app's chrome provider. */
export const registerChrome = (kind: string, p: ChromeProvider): void => nativeChrome.register(kind, p);

const toItem = (m: { id: string; label: string; sub?: string; icon?: string; danger?: boolean; checked?: boolean }): ChromeItem => ({ id: m.id, title: m.label, ...(m.sub ? { sub: m.sub } : {}), ...(m.danger ? { role: "destructive" as const } : {}), ...(m.checked ? { checked: true } : {}), ...(m.icon ? { icon: m.icon } : {}) });

/** The whole description for the current state of the stores: the app's body from its provider, and the shell's parts. Reads reactive state, so call it inside an effect. */
export function describe(kind: string, body: ChromeBody, scheme: "light" | "dark", revision: number): ChromeDescription {
  return {
    version: 1, revision, scheme, accent: accentOf(), appearance: appearanceOf(() => { nativeChrome.tick++; }),
    header: body.header,
    sidebar: {
      open: nativeChrome.open, title: appearance.prefs.brand,
      selectors: body.selectors, sections: body.sections,
      apps: switcherApps().map((a) => ({ id: a.kind, title: a.name, ...(a.icon ? { icon: a.icon } : {}), selected: a.kind === kind })),
      account: { id: "account", initial: [...self.name.trim()][0]?.toUpperCase() ?? "?", title: self.name, menu: accountItems().map(toItem) },
      search: body.search,
    },
  };
}

/** The row the shell last chose (a `row` event) and when: its selection echo is not worth a full apply, and while rows keep arriving the shell is stepping through the list (a key repeating). */
let lastRow: { id: string; at: number } | null = null;
/** A key repeat is over when no row event came for this long. */
const REPEAT_MS = 400;
/** While a key repeats, a publication that only moves counts waits so long since the last one that was sent. */
const COUNT_EVERY_MS = 1000;

/** Routes an event from the shell: the shell's own (app, account, sidebar, search) here, the rest to the app's provider. Returns whether anything answered it. */
export function routeEvent(kind: string, ev: ChromeEvent): boolean {
  if (ev.type === "row" && ev.id) { lastRow = { id: ev.id, at: performance.now() }; }
  switch (ev.type) {
    case "app": if (ev.id) navigate(`/${ev.id}`); return true;
    case "account": if (ev.id && pickAccountItem(ev.id)) return true; return false;
    case "sidebar": nativeChrome.open = !!ev.open; return true;
    case "search": nativeChrome.query = ev.text ?? ""; return true;
    case "window": if (ev.id && ev.from !== undefined && ev.to !== undefined) { nativeChrome.windows = { ...nativeChrome.windows, [ev.id]: [ev.from, ev.to] }; return nativeChrome.providerFor(kind)?.handle(ev) ?? true; } return true;
    case "menu": if (ev.id && !ev.rowId && pickAccountItem(ev.id)) return true; break;
  }
  return nativeChrome.providerFor(kind)?.handle(ev) ?? false;
}

/**
 * Starts publishing (once, after the first paint): an effect that rebuilds the description whenever the stores it reads change, and sends it at the next frame; and the listener for the shell's events. `appKind` says which app
 * is open (its provider supplies the body). A no-op without the bridge's `chrome`, and the reply `native: false` leaves the page as it is.
 */
/**
 * Before the first paint: when the shell can draw this contract (the bridge has `chrome` and a common version), the page is provisionally in native chrome: `data-native-chrome-pending` on the root hides the page's own sidebar,
 * drawer, header, wallpaper and card frame (native.css), so nothing of them is ever drawn beside the shell's, whatever the route or the stored drawer says. The answer to the first publication settles it: `native: true` keeps it
 * (`data-native-chrome-page`), anything else (a refusal, an error, no answer for `PENDING_MS`) withdraws it and the page draws its own. Decided from the bridge's presence, not from a reply.
 */
export function earlyNativeChrome(): void {
  if (!chromeCompatible()) return;
  const root = document.documentElement;
  root.setAttribute("data-native-chrome-pending", "");
  setTimeout(() => { if (!root.hasAttribute("data-native-chrome-page")) root.removeAttribute("data-native-chrome-pending"); }, PENDING_MS);
}
/** How long the page waits for the shell's first answer before it draws its own chrome. */
const PENDING_MS = 5000;

export function startNativeChrome(appKind: () => string): () => void {
  const b = bridge();
  if (!b || !chromeCompatible(b)) return () => {}; // no call, or a contract the shell cannot draw: the page keeps its own chrome
  let frame = 0, latest: ChromeDescription | null = null, lastSent = "", lastWire: ChromeDescription | null = null, lastAt = 0, held = 0;
  const send = async () => {
    frame = 0;
    const d = latest; if (!d) return;
    const t0 = performance.now();
    try {
      const wire = toSchema(d) as ChromeDescription;
      const same = JSON.stringify({ ...wire, revision: 0 });
      if (same === lastSent) return; // nothing in the description changed (a republish for a store that moved something not published): send nothing
      if (lastWire) {
        // The shell already shows the selection it made: an echo of only that is recorded as applied and not sent.
        if (lastRow && performance.now() - lastRow.at < 2000 && selectionEchoOnly(lastWire, wire, lastRow.id)) { lastSent = same; lastWire = wire; return; }
        // Under churn the counts move several times a second: while the shell steps through the list, a count-only change waits, at most one a second.
        if (lastRow && performance.now() - lastRow.at < REPEAT_MS && performance.now() - lastAt < COUNT_EVERY_MS && countsOnly(lastWire, wire)) {
          if (!held) held = window.setTimeout(() => { held = 0; if (!frame) frame = requestAnimationFrame(() => void send()); }, COUNT_EVERY_MS - (performance.now() - lastAt));
          return;
        }
      }
      lastSent = same; lastWire = wire; lastAt = performance.now();
      const r = await b.chrome!(wire);
      const ms = performance.now() - t0;
      nativeChrome.lat.push(ms); if (nativeChrome.lat.length > 200) nativeChrome.lat.shift();
      (window as unknown as { __chromeStats?: unknown }).__chromeStats = { published: d.revision, last: Math.round(ms * 10) / 10, avg: Math.round((nativeChrome.lat.reduce((x, y) => x + y, 0) / nativeChrome.lat.length) * 10) / 10 };
      if ([1, 3, 10, 30].includes(nativeChrome.lat.length)) { try { b.log?.(`chrome publish round trip: last ${ms.toFixed(1)} ms, avg ${(nativeChrome.lat.reduce((x, y) => x + y, 0) / nativeChrome.lat.length).toFixed(1)} ms over ${nativeChrome.lat.length}`); } catch { /* ignored */ } }
      const on = answerIsNative(r);
      if (!on && r?.reason) { try { b.log?.(`chrome refused: ${r.reason}${r.supported ? ` (shell draws ${r.supported.join(", ")})` : ""}`); } catch { /* ignored */ } }
      if (on !== nativeChrome.native) nativeChrome.native = on;
      document.documentElement.toggleAttribute("data-native-chrome-page", on);
      document.documentElement.removeAttribute("data-native-chrome-pending"); // the answer settles it: native keeps `-page`, a refusal draws the page's own
    } catch (e) { document.documentElement.removeAttribute("data-native-chrome-pending"); try { b.log?.(`chrome publish failed: ${String((e as Error)?.message ?? e).slice(0, 300)}`); } catch { /* ignored */ } /* the shell did not answer: keep drawing our own */ }
  };
  const stop = $effect.root(() => {
    $effect(() => {
      const kind = appKind(), p = nativeChrome.providerFor(kind);
      if (!p) return;
      void nativeChrome.tick;
      const body = p.describe(nativeChrome.query);
      const scheme = (document.documentElement.dataset.scheme === "dark" ? "dark" : "light") as "light" | "dark";
      latest = describe(kind, body, scheme, nativeChrome.nextRevision());
      if (!frame) frame = requestAnimationFrame(() => void send());
    });
  });
  const onEvent = (e: Event) => {
    const ev = (e as CustomEvent<ChromeEvent>).detail;
    try { b.log?.(`chrome event ${ev.type} ${ev.id ?? ""} after ${Date.now() - ev.sentAt} ms`); } catch { /* ignored */ }
    const handled = routeEvent(appKind(), ev);
    if (handled && (ev.type === "row" || ev.type === "app")) nativeChrome.open = false; // choosing something closes the sidebar
  };
  addEventListener("calternal:chrome", onEvent);
  return () => { removeEventListener("calternal:chrome", onEvent); stop(); clearTimeout(held); if (frame) cancelAnimationFrame(frame); };
}

/** The most rows sent for a section that is not windowed, and the margin kept around the window the shell shows. */
const WHOLE = 80, MARGIN = 20, MAX_WINDOW = 60;
/**
 * A long section as a window of its rows: all of them when the list is short, else the rows the shell asked for (`window` event) plus a margin, with `count` and `offset` (the shell then draws a list of one fixed row height and
 * keeps spacers for the rest). Reads `nativeChrome.windows`, so a new window republishes.
 */
export function windowed<T>(sectionId: string, all: T[]): { rows: T[]; count?: number; offset?: number } {
  return windowedOf(sectionId, all, (x) => x);
}
/** The window last published for each section (rows `[a, b)`): kept while the shell's request stays well inside it, so the published rows do not slide with every key press. */
const published = new Map<string, [number, number]>();
/** How close to a window's edge (rows) the request may come before the window moves, when the list continues beyond that edge. */
const EDGE = 24;
/**
 * As `windowed`, but the rows are built from `items` only for the window (a thousand threads cost a window of rows, not a thousand). The window is STABLE: the last one stays while the shell's request lies inside it with
 * up to `EDGE` rows (a quarter of what the request leaves free) to spare on any side the list continues past (or the request is wider than the window can leave room for and still lies within it); only then does it move, to one of `MAX_WINDOW` rows centred on the request.
 * A held arrow key that scrolls the shell's list by a row at a time therefore republishes no rows until the cursor nears the edge (the shell lays out every row it is sent: a slid window cost 55 to 80 ms frames).
 */
export function windowedOf<T, R>(sectionId: string, items: T[], build: (t: T) => R): { rows: R[]; count?: number; offset?: number } {
  const all = items;
  if (all.length <= WHOLE) { published.delete(sectionId); return { rows: all.map(build) }; }
  const [from, to] = nativeChrome.windows[sectionId] ?? [0, WHOLE / 2];
  const cur = published.get(sectionId);
  const edge = Math.max(0, Math.min(EDGE, Math.floor((MAX_WINDOW - (to - from)) / 4)));
  const inside = (w: [number, number]) => w[1] <= all.length && (w[0] === 0 ? from >= 0 : from - w[0] >= edge) && (w[1] === all.length ? to <= w[1] : w[1] - to >= edge) && to <= w[1];
  if (cur && inside(cur)) return { rows: all.slice(cur[0], cur[1]).map(build), count: all.length, offset: cur[0] };
  const mid = Math.floor((from + to) / 2);
  const a = Math.max(0, Math.min(all.length - MAX_WINDOW, mid - MAX_WINDOW / 2)), b = Math.min(all.length, a + MAX_WINDOW);
  published.set(sectionId, [a, b]);
  return { rows: all.slice(a, b).map(build), count: all.length, offset: a };
}
