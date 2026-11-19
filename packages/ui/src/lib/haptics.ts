/**
 * Haptics through the native shell, when there is one. A web page cannot vibrate on iPhone; the iPhone shell gives its web view `window.calternalNative.haptic(style)` (docs/ios-shell.md), and this is the one door to it:
 * in a browser (no shell) every call does nothing, so the web app is unchanged. Used where Apple's guidelines call for a haptic and nowhere else (docs/native-feel.md, rule 13): `selection` when a value or choice changes,
 * `success`, `warning` and `error` for outcomes. Never in bursts: calls closer than 80 ms apart are dropped; a person can turn them off with `localStorage["cal.haptics"] = "0"` (a Settings switch is a later, separate piece).
 */
export type HapticKind = "selection" | "light" | "medium" | "heavy" | "soft" | "rigid" | "success" | "warning" | "error";

interface NativeBridge { haptic?: (style: HapticKind) => Promise<unknown> }
let last = 0;

/** Whether the page runs inside the native shell (the bridge exists). */
export const inNativeShell = (): boolean => typeof window !== "undefined" && !!(window as unknown as { calternalNative?: NativeBridge }).calternalNative;

/** Plays one haptic in the native shell; does nothing in a browser, with the setting off, or when called again within 80 ms. A failure of the bridge is ignored (a haptic is never worth an error). */
export function haptic(kind: HapticKind): void {
  if (typeof window === "undefined") return;
  const b = (window as unknown as { calternalNative?: NativeBridge }).calternalNative;
  if (!b?.haptic) return;
  try { if (localStorage.getItem("cal.haptics") === "0") return; } catch { /* storage is off: on */ }
  const now = performance.now();
  if (now - last < 80) return;
  last = now;
  try { void Promise.resolve(b.haptic(kind)).catch(() => {}); } catch { /* ignored */ }
}
