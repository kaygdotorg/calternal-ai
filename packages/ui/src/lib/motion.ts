/**
 * Reading motion tokens in script. A computed custom property comes back as the browser normalised it
 * (`--dur-base` is ".22s", not "220ms"), so `parseFloat` alone reads 0.22 ms and every animation becomes instant.
 * These helpers are the one way to read a `--dur-*` or `--ease-*` token; nothing else in the apps parses them.
 */

/** A CSS time (`340ms`, `0.19s`, a bare number is ms) in milliseconds; `fallback` when it is not one. */
export function cssMs(value: string, fallback: number): number {
  const m = /^\s*(-?[\d.]+)\s*(ms|s)?\s*$/.exec(value);
  if (!m) return fallback;
  const n = parseFloat(m[1]!);
  return Number.isFinite(n) ? (m[2] === "s" ? n * 1000 : n) : fallback;
}

/** A duration token (`--dur-base`) in milliseconds, `fallback` when it is unset. Cost: one computed-style read; call on change, not per frame. */
export function tokenMs(name: string, fallback: number): number {
  return cssMs(getComputedStyle(document.documentElement).getPropertyValue(name), fallback);
}

/** An easing token (`--ease-morph`) as a CSS easing string, `fallback` when it is unset. */
export function tokenEase(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}
