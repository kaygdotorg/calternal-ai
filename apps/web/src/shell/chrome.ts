/**
 * The page's own chrome on a phone, ported from the existing app (lib/home-screen.ts, packages/ui page-chrome.ts, origin/dev): what the browser draws around the page and what the page must leave room for.
 *
 *  - Safe areas: the shell pads regions by the notch and the home indicator (`--safe-*` tokens, docs/ui-shell.md "Safe areas"); apps and plugins get that padding from the shell and never read `env()` themselves.
 *  - The bars take the wallpaper's colour: Safari tints its toolbar and status bar from `theme-color`; it is set to the colour at the top of the page (the card's top edge, the wallpaper copy under its veil), and only
 *    written when it really changes (some browsers repaint their chrome on every write, even of the same value, and it shows as a flicker up there).
 *  - A home-screen app on iOS 26 and 27 is laid out one status bar shorter than the screen (`100dvh`, `innerHeight` and `position: fixed` boxes stop about 62 px above the bottom: WebKit bugs 301994 and 301108),
 *    so no unit is trusted alone: a zero-width fixed probe measures the layout viewport, and where the page runs under the status bar the screen's own size counts too. The tallest sizes the app (`--vh` on the root);
 *    `data-home-screen` marks it. Only a home-screen app, never a Safari tab, Android or a desktop browser.
 *  - The keyboard: iOS keeps the layout viewport under the keyboard, so a field at the bottom hides behind it. While a text field is focused, the part of the screen the keyboard covers goes to `--kb` and the shell
 *    lifts by it. Where the browser resizes the page itself (Android Chrome, `interactive-widget=resizes-content`) the difference is zero and nothing happens.
 */

/** Opened from the home screen (or installed elsewhere), not in a browser tab. */
export function installed(): boolean {
  return matchMedia("(display-mode: standalone)").matches || (navigator as Navigator & { standalone?: boolean }).standalone === true;
}

/** An iPhone or iPad home-screen app: touch WebKit, installed. Tests turn it on anywhere with `globalThis.__homeScreen = true` before the app loads. */
function onHomeScreen(): boolean {
  if ((globalThis as { __homeScreen?: boolean }).__homeScreen) return true;
  return installed() && typeof CSS !== "undefined" && CSS.supports("-webkit-touch-callout", "none");
}

const root = () => document.documentElement;
/** Sets (or, with null, removes) a custom property on the root, only if it changes: a write the page does not need would restyle every element. */
export function setRootVar(name: string, value: string | null): void {
  const s = root().style;
  if (value === null) { if (s.getPropertyValue(name) !== "") s.removeProperty(name); } else if (s.getPropertyValue(name) !== value) s.setProperty(name, value);
}
/** Sets the browser's bar colour: every `theme-color` meta tag to `color` (a hex colour: Safari does not read the space-separated `rgb()` form, and then keeps the fixed one from index.html, which showed as a separate band above the page), only where it differs (see the note above on flicker). */
export function setThemeColor(color: string): void {
  for (const m of document.querySelectorAll('meta[name="theme-color"]')) if (m.getAttribute("content") !== color) m.setAttribute("content", color);
  // iOS 26 and 27 Safari ignore `theme-color` and tint the status bar and toolbar from the page's own background at the edges (checked on the Simulator: a body background reaches under the status bar and the bars take it),
  // so the root's background is the same solved colour: the app runs edge to edge with no separate band.
  setRootVar("--bars", color); // read by `:root:root { background-color: var(--bars) }` (styles/base.css); the page's own `body` background would otherwise colour the canvas and show as a band
}

/** The screen's own height when the app covers the whole screen (under the status bar, or turned on its side), else 0. */
function screenTall(safeTop: number): number {
  const short = Math.min(screen.width, screen.height), long = Math.max(screen.width, screen.height);
  const portrait = innerHeight >= innerWidth;
  if (!(safeTop > 0 || !portrait)) return 0;
  if (Math.abs(innerWidth - (portrait ? short : long)) > 1) return 0; // a window beside another (an iPad), not a full-screen app
  return portrait ? long : short;
}

let started = false;
/**
 * Starts following the viewport: the home-screen height and the keyboard. Call once, after the first paint. Cost: one `ResizeObserver` on a zero-width probe, `resize` and `visualViewport` listeners that write
 * a custom property only when its value changes.
 */
export function startChrome(): void {
  if (started) return;
  started = true;
  if (onHomeScreen()) {
    root().dataset.homeScreen = "";
    const probe = document.createElement("div");
    probe.setAttribute("aria-hidden", "true");
    // Zero wide and invisible: no browser takes its colour for its bars. Its top padding is the top safe area, read the way the page has it.
    probe.style.cssText = "position:fixed;top:0;bottom:0;left:0;width:0;padding-top:var(--safe-top);box-sizing:border-box;visibility:hidden;pointer-events:none;contain:strict";
    document.body.appendChild(probe);
    let height = 0;
    const fit = () => {
      const typing = !!document.activeElement && /^(INPUT|TEXTAREA)$/.test(document.activeElement.tagName);
      const h = Math.max(Math.round(probe.getBoundingClientRect().height), screenTall(parseFloat(getComputedStyle(probe).paddingTop) || 0));
      if (!h || h === height || (h < height && typing)) return; // the keyboard never makes the app shorter
      height = h;
      setRootVar("--vh", `${h}px`);
    };
    new ResizeObserver(fit).observe(probe);
    addEventListener("resize", fit, { passive: true });
  }
  const vv = window.visualViewport;
  if (vv && matchMedia("(pointer: coarse)").matches) {
    const kb = () => {
      const field = document.activeElement && /^(INPUT|TEXTAREA)$/.test(document.activeElement.tagName) || (document.activeElement as HTMLElement | null)?.isContentEditable;
      const covered = field ? Math.max(0, Math.round(innerHeight - vv.height - vv.offsetTop)) : 0;
      setRootVar("--kb", covered > 80 ? `${covered}px` : null); // under 80 px is the browser's own bar, not a keyboard
    };
    vv.addEventListener("resize", kb, { passive: true });
    vv.addEventListener("scroll", kb, { passive: true });
    addEventListener("focusin", kb, { passive: true });
    addEventListener("focusout", () => setTimeout(kb, 50), { passive: true });
  }
}
