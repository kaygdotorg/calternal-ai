/**
 * Input modality, tracked on <html> as `data-input="key" | "pointer"` (pointer covers mouse, pen and touch).
 *
 * Why it exists (owner rules, 2026-11-17): a focus ring is for keyboard focus only, never from a mouse or touch, even in a text field where
 * `:focus-visible` would always match; and controls animate when used by pointer or touch, never when invoked from the keyboard (a key press
 * applies the end state, so typing, tabbing and Enter feel instant). base.css reads the attribute for both rules.
 *
 * What is not the keyboard (owner, 2026-11-18: a ring appeared on a chip after a mouse click): a modifier on its own, and a Meta or Control chord with a character (Cmd-Shift-4, Cmd-Tab, Ctrl-C).
 * The operating system and the browser answer those without touching the focused control, so they must not turn a ring on for it.
 *
 * Cost: three passive listeners on the document; one attribute write when the modality changes, never per event.
 */

/** The last way the person drove the page. */
export type Modality = "key" | "pointer";

const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "AltGraph", "Meta", "OS", "CapsLock", "Fn", "FnLock"]);

let mode: Modality = "pointer";

function set(m: Modality): void {
  if (m === mode && document.documentElement.dataset.input === m) return;
  mode = m;
  document.documentElement.dataset.input = m;
}

/** The current modality. */
export const modality = (): Modality => mode;

/** Starts tracking. Call once at start-up. Any other key is the keyboard. */
export function initInputModality(): void {
  document.documentElement.dataset.input = mode;
  // Typing a character into a text field is not navigation, and flipping the attribute restyles the whole page (about 15 ms at 4x CPU on the composer's first key): that waits for an idle
  // moment between keys instead of landing in the key's own frame. Any other key (Tab, arrows, Enter, Escape, shortcuts) flips at once, as does the pointer.
  let later = 0;
  const cancel = () => { if (later) { (window.cancelIdleCallback ?? clearTimeout)(later); later = 0; } };
  const typing = (e: KeyboardEvent) => e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && (e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement || (e.target as HTMLElement | null)?.isContentEditable === true);
  addEventListener("keydown", (e) => {
    if (e.isComposing || MODIFIER_KEYS.has(e.key) || ((e.metaKey || e.ctrlKey) && e.key.length === 1)) return;
    if (mode !== "key" && typing(e)) { if (!later) later = (window.requestIdleCallback ?? ((f: () => void) => setTimeout(f, 80)))(() => { later = 0; set("key"); }, { timeout: 400 } as never) as number; return; }
    cancel(); set("key");
  }, { capture: true, passive: true });
  addEventListener("pointerdown", () => { cancel(); set("pointer"); }, { capture: true, passive: true });
  // A mouse that merely moves does not end keyboard use; real movement after a key press does after a few pixels.
  let x = -1, y = -1;
  addEventListener("pointermove", (e) => {
    if (mode === "pointer") return;
    if (x >= 0 && Math.hypot(e.clientX - x, e.clientY - y) > 4) set("pointer");
    x = e.clientX; y = e.clientY;
  }, { capture: true, passive: true });
}
