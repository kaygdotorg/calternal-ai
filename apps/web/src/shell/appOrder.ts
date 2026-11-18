/** The apps in the order the person chose (`order` lists app kinds first to last); apps it does not name keep their own order after the named ones, and names that are not apps are ignored. */
export function applyOrder<T extends { kind: string }>(apps: T[], order: string[]): T[] {
  const rank = (a: T) => { const i = order.indexOf(a.kind); return i < 0 ? order.length + apps.indexOf(a) : i; };
  return [...apps].sort((a, b) => rank(a) - rank(b));
}
/** `order` with `kind` moved one place up (`-1`) or down (`1`), written out in full from `all` (every app's kind in the current order) so the result lists every app. */
export function move(all: string[], kind: string, by: -1 | 1): string[] {
  const i = all.indexOf(kind), j = i + by;
  if (i < 0 || j < 0 || j >= all.length) return all;
  const out = [...all]; [out[i], out[j]] = [out[j]!, out[i]!];
  return out;
}
