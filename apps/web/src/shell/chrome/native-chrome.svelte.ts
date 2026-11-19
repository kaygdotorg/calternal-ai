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
import { accountItems, pickAccountItem } from "../accountMenu";
import { self } from "../self.svelte";
import { switcherApps } from "../switcherApps.svelte";
import { toDraft1 } from "./draft1";
import type { ChromeBody, ChromeDescription, ChromeEvent, ChromeItem, ChromeProvider } from "./types";

/** Which form of the contract the shell decodes: "draft-1" (the first draft, reduced by `draft1.ts`) until the shell takes the agreed fields, then "v2" (the full description). */
export const CONTRACT: "draft-1" | "v2" = "draft-1";

interface Bridge { chrome?: (d: ChromeDescription | null) => Promise<{ native: boolean }>; log?: (s: string) => unknown }
const bridge = (): Bridge | undefined => (typeof window === "undefined" ? undefined : (window as unknown as { calternalNative?: Bridge }).calternalNative);

/** Whether the page can publish chrome at all: the bridge has the call (only the iPhone shell, and only a build that has the experiment). */
export const chromeAvailable = (): boolean => typeof bridge()?.chrome === "function";

class NativeChrome {
  /** The shell drew the chrome for the last publication (it said `native: true`). */
  native = $state(false);
  /** Whether the person has the native sidebar open (the shell tells us when it changes). */
  open = $state(false);
  /** The text in the native search field. */
  query = $state("");
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

const toItem = (m: { id: string; label: string; sub?: string; icon?: string; danger?: boolean; checked?: boolean }): ChromeItem => ({ id: m.id, title: m.label, ...(m.sub ? { sub: m.sub } : {}), ...(m.danger ? { role: "destructive" as const } : {}), ...(m.checked ? { checked: true } : {}), ...(m.icon === "user" ? { symbol: "person.crop.circle" } : m.icon === "settings" ? { symbol: "gearshape" } : m.icon === "signout" ? { symbol: "rectangle.portrait.and.arrow.right" } : {}) });
const SYMBOL: Record<string, string> = { threads: "bubble.left", notes: "doc.text", settings: "gearshape" };

/** The whole description for the current state of the stores: the app's body from its provider, and the shell's parts. Reads reactive state, so call it inside an effect. */
export function describe(kind: string, body: ChromeBody, scheme: "light" | "dark", revision: number): ChromeDescription {
  return {
    version: 1, revision, scheme, accent: accentOf(),
    header: body.header,
    sidebar: {
      open: nativeChrome.open, title: appearance.prefs.brand,
      selectors: body.selectors, sections: body.sections,
      apps: switcherApps().map((a) => ({ id: a.kind, title: a.name, ...(SYMBOL[a.kind] ? { symbol: SYMBOL[a.kind] } : {}), selected: a.kind === kind })),
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
  if (!b?.chrome) return () => {};
  let frame = 0, latest: ChromeDescription | null = null;
  const send = async () => {
    frame = 0;
    const d = latest; if (!d) return;
    const t0 = performance.now();
    try {
      const r = await b.chrome!((CONTRACT === "draft-1" ? toDraft1(d) : d) as ChromeDescription);
      const ms = performance.now() - t0;
      nativeChrome.lat.push(ms); if (nativeChrome.lat.length > 200) nativeChrome.lat.shift();
      (window as unknown as { __chromeStats?: unknown }).__chromeStats = { published: d.revision, last: Math.round(ms * 10) / 10, avg: Math.round((nativeChrome.lat.reduce((x, y) => x + y, 0) / nativeChrome.lat.length) * 10) / 10 };
      if ([1, 3, 10, 30].includes(nativeChrome.lat.length)) { try { b.log?.(`chrome publish round trip: last ${ms.toFixed(1)} ms, avg ${(nativeChrome.lat.reduce((x, y) => x + y, 0) / nativeChrome.lat.length).toFixed(1)} ms over ${nativeChrome.lat.length}`); } catch { /* ignored */ } }
      if (r.native !== nativeChrome.native) { nativeChrome.native = r.native; document.documentElement.toggleAttribute("data-native-chrome-page", r.native); }
    } catch (e) { try { b.log?.(`chrome publish failed: ${String((e as Error)?.message ?? e).slice(0, 300)}`); } catch { /* ignored */ } /* the shell did not answer: keep drawing our own */ }
  };
  const stop = $effect.root(() => {
    $effect(() => {
      const kind = appKind(), p = nativeChrome.providerFor(kind);
      if (!p) return;
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
