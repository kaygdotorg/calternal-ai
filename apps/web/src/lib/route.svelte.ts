/**
 * The address bar as state. Addresses are plain paths rooted at `/` (docs/addresses.md): `/<kind>/<id>` and, inside it, `/<block-id>`.
 * Transient parameters (the open right-card panel, a search) are in the query and never identify anything.
 * Links are ordinary `<a href>`; `interceptLinks` turns same-origin clicks into history navigation without a reload.
 */

/**
 * Where the app is mounted: "/" normally, "/ai/" on the preview host. Addresses inside the app never contain it; links and the address bar do.
 * Set by the build (`CAL_BASE`), read from Vite's `BASE_URL`.
 */
const BASE = import.meta.env.BASE_URL.replace(/\/$/, "");

/** An app address (`/threads/abc`) as it appears in the address bar and in an `<a href>`. */
export const href = (path: string): string => BASE + path;

const inApp = (pathname: string): string => (BASE && pathname.startsWith(BASE) ? pathname.slice(BASE.length) || "/" : pathname);

/** The current location. Read it in markup or a `$derived` and it stays current. */
export const route = $state({ path: inApp(location.pathname), query: new URLSearchParams(location.search) });

/** `/threads/abc` becomes `["threads", "abc"]`. */
export const segments = (): string[] => route.path.split("/").filter(Boolean);

function sync(): void {
  route.path = inApp(location.pathname);
  route.query = new URLSearchParams(location.search);
}

/** Goes to `to` (a path with an optional query) without a reload. */
export function navigate(to: string, replace = false): void {
  const full = href(to);
  if (full === location.pathname + location.search) return;
  history[replace ? "replaceState" : "pushState"](replace ? history.state : null, "", full); // a query change keeps the entry's state (the shell marks entries that show the list or the side panel)
  sync();
}

/** Sets or removes one query parameter on the current path. */
export function setQuery(key: string, value: string | null): void {
  const q = new URLSearchParams(route.query);
  if (value === null) q.delete(key);
  else q.set(key, value);
  const s = q.toString();
  navigate(route.path + (s ? `?${s}` : ""), true);
}

/** Starts following the browser's back and forward buttons and turns clicks on same-origin links into navigations. Call once. */
export function interceptLinks(): void {
  addEventListener("popstate", sync);
  document.addEventListener("click", (e) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
    if (!a || a.target === "_blank" || a.origin !== location.origin || a.hasAttribute("download")) return;
    e.preventDefault();
    navigate(inApp(a.pathname) + a.search);
  });
}
