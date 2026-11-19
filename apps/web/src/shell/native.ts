/**
 * The page's half of the native shell's bridge (`window.calternalNative`, docs/ios-shell.md on lane/ios-shell): present only inside the iPhone shell, so every call is behind a check and the web app is unchanged in a browser.
 * `appearance` tells the shell the scheme (status bar text, keyboard look) and the colour behind the page, on every theme change; `ready` tells it the first frame is painted so it can fade its launch cover.
 * Haptics are in `@calternal/ui` (`haptic`). A rejected call is ignored: the shell not answering must never break the page.
 */
interface Bridge { appearance?: (o: { scheme: "light" | "dark"; background: string }) => Promise<unknown>; ready?: () => Promise<unknown> }
const bridge = (): Bridge | undefined => (window as unknown as { calternalNative?: Bridge }).calternalNative;
let lastAppearance = "";

/** Reports the scheme and the colour behind the page to the shell, when it changed since the last report. */
export function nativeAppearance(scheme: "light" | "dark", background: string): void {
  const b = bridge();
  if (!b?.appearance) return;
  const key = `${scheme}|${background}`;
  if (key === lastAppearance) return;
  lastAppearance = key;
  try { void Promise.resolve(b.appearance({ scheme, background })).catch(() => {}); } catch { /* ignored */ }
}

let said = false;
/** Tells the shell the first real frame is painted (once). */
export function nativeReady(): void {
  if (said) return;
  said = true;
  const b = bridge();
  try { if (b?.ready) void Promise.resolve(b.ready()).catch(() => {}); } catch { /* ignored */ }
}
