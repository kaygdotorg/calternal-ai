/**
 * Input modality, tracked on <html> as `data-input="key" | "pointer"` (pointer covers mouse, pen and touch).
 *
 * Why it exists (owner rules, 2026-11-17): a focus ring is for keyboard focus only, never from a mouse or touch, even in a text field where
 * `:focus-visible` would always match; and controls animate when used by pointer or touch, never when invoked from the keyboard (a key press
 * applies the end state, so typing, tabbing and Enter feel instant). base.css reads the attribute for both rules.
 *
 * Cost: three passive listeners on the document; one attribute write when the modality changes, never per event.
 */

/** The last way the person drove the page. */
export type Modality = "key" | "pointer";

let mode: Modality = "pointer";

function set(m: Modality): void {
  if (m === mode && document.documentElement.dataset.input === m) return;
  mode = m;
  document.documentElement.dataset.input = m;
}

/** The current modality. */
export const modality = (): Modality => mode;

/** Starts tracking. Call once at start-up. Modifier-only keys and keys that carry no navigation (e.g. the shift of a capital letter) still count: any key is the keyboard. */
export function initInputModality(): void {
  document.documentElement.dataset.input = mode;
  addEventListener("keydown", (e) => { if (!e.isComposing) set("key"); }, { capture: true, passive: true });
  addEventListener("pointerdown", () => set("pointer"), { capture: true, passive: true });
  // A mouse that merely moves does not end keyboard use; real movement after a key press does after a few pixels.
  let x = -1, y = -1;
  addEventListener("pointermove", (e) => {
    if (mode === "pointer") return;
    if (x >= 0 && Math.hypot(e.clientX - x, e.clientY - y) > 4) set("pointer");
    x = e.clientX; y = e.clientY;
  }, { capture: true, passive: true });
}
