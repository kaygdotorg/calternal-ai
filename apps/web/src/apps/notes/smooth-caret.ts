/**
 * The smooth text cursor (SC1 to SC6 of the existing app's `ops/orchestrator/notes/mockup-float-layout.md`; Word calls it "smooth typing", VS Code `cursorSmoothCaretAnimation`). Ported from the
 * mockup's `SmoothCaret.svelte` (origin/feat/appwindow3), adapted from a contenteditable overlay to a CodeMirror layer:
 *
 *  SC1/SC2  the caret glides to where it goes, while typing and moving, in `GLIDE_MS` on the app's `--ease-out`; a move that comes within `SNAP_MS` of the last one snaps (no queued glides), so the
 *           caret is never more than one character behind (the pitfall of a fixed glide, vscode #295648).
 *  SC3      blink: a smooth fade. It is solid while keys are typed (`:root[data-typing]`, which lib/input.ts holds until 800 ms after the last key) and starts fading after a pause. A move that is not
 *           typing (a click, an arrow up or down) restarts the blink from solid, by swapping the animation's name: typing never touches it, because restarting an animation per key costs (base.css).
 *  SC4      a selection made with the keyboard grows smoothly (its boxes ease their width, `data-glide`); one made with the pointer follows the pointer exactly.
 *  SC5      reduced motion, a touch screen or `data-cursor="off"` on an ancestor (the setting "Animated cursor"): none of this exists and CodeMirror's own caret is used.
 *  SC6      only this caret's own element moves, by `transform`; per key the cost is one `coordsAtPos` read (the measure step CodeMirror already runs for its own caret) and one style write.
 *
 * Other people's cursors in a live note are the live extension's own widgets and are not touched; only this person's primary caret is replaced.
 */
import { EditorView, layer, type LayerMarker } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import "./smooth-caret.css";

/** The glide's length, inside the agreed 80 to 100 ms. */
export const GLIDE_MS = 90;
/** A move this soon after the last one snaps instead of gliding. */
export const SNAP_MS = 70;

let flip = 0;

class Caret implements LayerMarker {
  constructor(readonly x: number, readonly y: number, readonly h: number, readonly snap: boolean, readonly typing: boolean) {}
  eq(o: Caret) { return o.x === this.x && o.y === this.y && o.h === this.h; }
  draw() { const e = document.createElement("div"); e.className = "cm-smooth-caret"; this.set(e, true); return e; }
  update(e: HTMLElement, prev: LayerMarker) { this.set(e, false); return prev instanceof Caret; }
  private set(e: HTMLElement, fresh: boolean) {
    e.style.transform = `translate3d(${this.x}px, ${this.y}px, 0)`;
    e.style.height = `${this.h}px`;
    e.classList.toggle("snap", this.snap || fresh);
    // Typing leaves the blink alone (it is paused by `data-typing`); any other move restarts it from solid.
    if (!this.typing) e.dataset.k = String((flip ^= 1));
  }
}

/** Is the smooth caret allowed here: not under reduced motion, not on a touch screen, not switched off. */
const allowed = (view: EditorView) =>
  !matchMedia("(prefers-reduced-motion: reduce)").matches && !matchMedia("(pointer: coarse)").matches && !view.dom.closest("[data-cursor='off']");

/** The smooth caret and keyboard-selection glide for a block editor. Add it to the editor's `extensions`. */
export const smoothCaret: Extension = (() => {
  let stamp = 0; // when the pending move was first seen (an event's time, not the frame's: a slow frame must not turn quick typing into slow typing)
  let at = -1e9; // time of the last move that was drawn
  const caret = layer({
    above: true,
    class: "cm-smooth-caret-layer",
    markers(view) {
      const { main } = view.state.selection;
      const on = allowed(view);
      view.dom.toggleAttribute("data-smooth", on);
      if (!on || !main.empty || view.root.activeElement !== view.contentDOM) return [];
      const c = view.coordsAtPos(main.head, main.assoc || 1);
      if (!c) return [];
      const r = view.scrollDOM.getBoundingClientRect();
      const x = Math.round((c.left - r.left + view.scrollDOM.scrollLeft) * 10) / 10;
      const y = Math.round((c.top - r.top + view.scrollDOM.scrollTop) * 10) / 10;
      const now = stamp || performance.now();
      stamp = 0;
      const snap = now - at < SNAP_MS;
      at = now;
      return [new Caret(x, y, c.bottom - c.top, snap, document.documentElement.hasAttribute("data-typing"))];
    },
    update(u) {
      const moved = u.selectionSet || u.docChanged;
      if (moved) stamp ||= performance.now();
      // Keyboard selections glide, pointer ones follow the pointer (only a write when the mode changes).
      if (u.selectionSet && u.view.dom.hasAttribute("data-smooth")) {
        const kb = u.transactions.some((t) => t.isUserEvent("select") && !t.isUserEvent("select.pointer")) && !u.state.selection.main.empty;
        if (kb !== u.view.dom.hasAttribute("data-glide")) u.view.dom.toggleAttribute("data-glide", kb);
      }
      return moved || u.viewportChanged || u.focusChanged;
    },
  });
  const theme = EditorView.baseTheme({
    // The browser-drawn caret steps aside (this person's primary one only).
    "&[data-smooth] .cm-cursor-primary": { display: "none !important" },
    "&[data-smooth].cm-picking .cm-smooth-caret, &[data-smooth].cm-blocks-shown .cm-smooth-caret": { display: "none" },
    ".cm-smooth-caret": {
      position: "absolute", left: "0", top: "0", width: "2px", borderRadius: "1px", background: "var(--accent)", pointerEvents: "none", willChange: "transform",
      transition: `transform ${GLIDE_MS}ms var(--ease-out)`,
      animation: "cm-sc-blink-a 1.1s ease-in-out 520ms infinite backwards",
    },
    ".cm-smooth-caret[data-k='1']": { animationName: "cm-sc-blink-b" },
    ".cm-smooth-caret.snap": { transition: "none" },
    "@keyframes cm-sc-blink-a": { "0%, 38%": { opacity: 1 }, "62%, 88%": { opacity: 0.04 }, "100%": { opacity: 1 } },
    "@keyframes cm-sc-blink-b": { "0%, 38%": { opacity: 1 }, "62%, 88%": { opacity: 0.04 }, "100%": { opacity: 1 } },
    // A keyboard selection eases its boxes' width; the first appearance of a box grows out of its left edge.
    "&[data-glide] .cm-selectionBackground": { transition: `width ${GLIDE_MS}ms var(--ease-out), left ${GLIDE_MS}ms var(--ease-out)` },
    "@media (prefers-reduced-motion: reduce)": { ".cm-smooth-caret": { transition: "none", animation: "none" } },
  });
  return [caret, theme];
})();
