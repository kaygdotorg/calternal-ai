/**
 * Input modality, tracked on <html> as `data-input="key" | "pointer"` (pointer covers mouse, pen and touch).
 *
 * Why it exists (owner rules, 2026-11-17): a focus ring is for keyboard focus only, never from a mouse or touch, even in a text field where
 * `:focus-visible` would always match; and controls animate when used by pointer or touch, never when invoked from the keyboard (a key press
 * applies the end state, so typing, tabbing and Enter feel instant). base.css reads the attribute for both rules.
 *
 * What is not the keyboard (owner, 2026-11-18: a ring appeared on a chip after a mouse click): a modifier on its own, a Meta or Control chord with a character (Cmd-Shift-4, Cmd-Tab, Ctrl-C),
 * and any key but Tab, Up and Down inside a text field (typing is not navigation, and the change restyles the whole page; decorative animations pause while it is typed in instead).
 * The operating system and the browser answer chords without touching the focused control, so they must not turn a ring on for it.
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
  // Typing and editing in a text field are not navigation: only Tab, and Up and Down (a picker or a list), take focus to another control, so only they change the modality. Flipping the attribute on
  // <html> restyles the whole page (a 19 ms recalculation of 683 elements at 4x CPU, found with a style trace: the universal `[data-input="key"] *` rules), which landed on the composer's first
  // key and again on the key after every click. The page-wide key mode also froze every spinner and pulse as a side effect; a field's own keys now do that on purpose and cheaply (`data-typing`).
  let rest = 0, pending = 0;
  const inField = (t: EventTarget | null) => t instanceof HTMLTextAreaElement || (t instanceof HTMLInputElement && !["button", "checkbox", "radio", "range", "color", "file", "submit", "reset", "image"].includes(t.type)) || (t as HTMLElement | null)?.isContentEditable === true;
  addEventListener("keydown", (e) => {
    if (e.isComposing || MODIFIER_KEYS.has(e.key) || ((e.metaKey || e.ctrlKey) && e.key.length === 1)) return;
    if (inField(e.target)) {
      if (e.key !== "Tab" && e.key !== "ArrowUp" && e.key !== "ArrowDown") {
        // Decorative animations elsewhere on the page (a spinner, a pulse, a shimmer) pause while keys are typed and resume 800 ms after the last one: they were running through every keystroke's
        // frame (the invalidation trace shows each frame restyling them), which the page-wide key mode used to stop by accident.
        // The attribute is set after the first key's frame has been presented (a frame, then a task): the restyle of the animated elements is never inside the first key's frame.
        const root = document.documentElement;
        if (!root.hasAttribute("data-typing") && !pending) pending = requestAnimationFrame(() => { pending = 0; window.setTimeout(() => root.setAttribute("data-typing", ""), 0); });
        clearTimeout(rest);
        rest = window.setTimeout(() => root.removeAttribute("data-typing"), 800);
        return;
      }
    }
    set("key");
  }, { capture: true, passive: true });
  addEventListener("pointerdown", () => {
    set("pointer");
    clearTimeout(rest);
    document.documentElement.removeAttribute("data-typing");
  }, { capture: true, passive: true });
  // A mouse that merely moves does not end keyboard use; real movement after a key press does after a few pixels.
  let x = -1, y = -1;
  addEventListener("pointermove", (e) => {
    if (mode === "pointer") return;
    if (x >= 0 && Math.hypot(e.clientX - x, e.clientY - y) > 4) set("pointer");
    x = e.clientX; y = e.clientY;
  }, { capture: true, passive: true });
}
