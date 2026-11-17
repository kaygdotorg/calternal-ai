/**
 * The address bar as state. Addresses are plain paths rooted at `/` (docs/addresses.md): `/<kind>/<id>` and, inside it, `/<block-id>`.
 * Transient parameters (the open right-card panel, a search) are in the query and never identify anything.
 * Links are ordinary `<a href>`; `interceptLinks` turns same-origin clicks into history navigation without a reload.
 */

/** The current location. Read it in markup or a `$derived` and it stays current. */
export const route = $state({ path: location.pathname, query: new URLSearchParams(location.search) });

/** `/threads/abc` becomes `["threads", "abc"]`. */
export const segments = (): string[] => route.path.split("/").filter(Boolean);

function sync(): void {
  route.path = location.pathname;
  route.query = new URLSearchParams(location.search);
}

/** Goes to `to` (a path with an optional query) without a reload. */
export function navigate(to: string, replace = false): void {
  if (to === location.pathname + location.search) return;
  history[replace ? "replaceState" : "pushState"](null, "", to);
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
    navigate(a.pathname + a.search);
  });
}
