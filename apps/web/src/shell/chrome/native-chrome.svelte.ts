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
import type { ChromeBody, ChromeDescription, ChromeEvent, ChromeItem, ChromeProvider } from "./types";


interface Bridge { chrome?: (d: ChromeDescription | null) => Promise<{ native: boolean; contract?: number }>; log?: (s: string) => unknown; chromeContract?: number | number[] }
const bridge = (): Bridge | undefined => (typeof window === "undefined" ? undefined : (window as unknown as { calternalNative?: Bridge }).calternalNative);

/** The version of the chrome contract this page publishes (`version` in the description; contract/chrome-v1.schema.json). */
export const PAGE_CONTRACT = 1;

/**
 * The compatibility rule (docs/ui-shell.md, "Native chrome compatibility"): the page and the shell negotiate the chrome contract over the bridge, and on ANY mismatch native chrome stays off and the page draws its own, never a half-drawn
 * shell (the phone's shell and the server's page are not always updated together). The shell advertises the contract versions it can draw as `calternalNative.chromeContract` (a number or a list) and answers a publication with
 * `{native, contract}`. The page publishes only when it has the call and, if the shell advertises versions, ours is one of them; a shell that advertises nothing is one from before the negotiation: it is tried, and an answer
 * that is not clear (an error, no `native`, a different `contract`) is treated as off and withdrawn.
 */
export function chromeCompatible(b: Pick<Bridge, "chrome" | "chromeContract"> | undefined = bridge()): boolean {
  if (typeof b?.chrome !== "function") return false;
  const c = b.chromeContract;
  if (c === undefined) return true;
  return (Array.isArray(c) ? c : [c]).includes(PAGE_CONTRACT);
}

/** Whether the shell's answer to a publication means native chrome is on for this contract: `native` true, and a `contract` (when it names one) that is ours. */
export const answerIsNative = (r: { native?: boolean; contract?: number } | null | undefined): boolean => !!r && r.native === true && (r.contract === undefined || r.contract === PAGE_CONTRACT);

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

/** Routes an event from the shell: the shell's own (app, account, sidebar, search) here, the rest to the app's provider. Returns whether anything answered it. */
export function routeEvent(kind: string, ev: ChromeEvent): boolean {
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
export function startNativeChrome(appKind: () => string): () => void {
  const b = bridge();
  if (!b || !chromeCompatible(b)) return () => {}; // no call, or a contract the shell cannot draw: the page keeps its own chrome
  let frame = 0, latest: ChromeDescription | null = null;
  const send = async () => {
    frame = 0;
    const d = latest; if (!d) return;
    const t0 = performance.now();
    try {
      const r = await b.chrome!(toSchema(d) as ChromeDescription);
      const ms = performance.now() - t0;
      nativeChrome.lat.push(ms); if (nativeChrome.lat.length > 200) nativeChrome.lat.shift();
      (window as unknown as { __chromeStats?: unknown }).__chromeStats = { published: d.revision, last: Math.round(ms * 10) / 10, avg: Math.round((nativeChrome.lat.reduce((x, y) => x + y, 0) / nativeChrome.lat.length) * 10) / 10 };
      if ([1, 3, 10, 30].includes(nativeChrome.lat.length)) { try { b.log?.(`chrome publish round trip: last ${ms.toFixed(1)} ms, avg ${(nativeChrome.lat.reduce((x, y) => x + y, 0) / nativeChrome.lat.length).toFixed(1)} ms over ${nativeChrome.lat.length}`); } catch { /* ignored */ } }
      const on = answerIsNative(r);
      if (r.native === true && !on) void b.chrome!(null).catch(() => {}); // the shell drew it for a contract we do not speak: withdraw it, never leave it half drawn
      if (on !== nativeChrome.native) { nativeChrome.native = on; document.documentElement.toggleAttribute("data-native-chrome-page", on); }
    } catch (e) { try { b.log?.(`chrome publish failed: ${String((e as Error)?.message ?? e).slice(0, 300)}`); } catch { /* ignored */ } /* the shell did not answer: keep drawing our own */ }
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
  return () => { removeEventListener("calternal:chrome", onEvent); stop(); if (frame) cancelAnimationFrame(frame); };
}

/** The most rows sent for a section that is not windowed, and the margin kept around the window the shell shows. */
const WHOLE = 80, MARGIN = 20;
/**
 * A long section as a window of its rows: all of them when the list is short, else the rows the shell asked for (`window` event) plus a margin, with `count` and `offset` (the shell then draws a list of one fixed row height and
 * keeps spacers for the rest). Reads `nativeChrome.windows`, so a new window republishes.
 */
export function windowed<T>(sectionId: string, all: T[]): { rows: T[]; count?: number; offset?: number } {
  if (all.length <= WHOLE) return { rows: all };
  const [from, to] = nativeChrome.windows[sectionId] ?? [0, WHOLE / 2];
  const a = Math.max(0, from - MARGIN), b = Math.min(all.length, to + MARGIN);
  return { rows: all.slice(a, b), count: all.length, offset: a };
}
