/**
 * The composer's expand and collapse, drawn without a layout per frame (docs/threads-ui.md, "Composer expand and collapse"). The state flips at once: the composer is laid out in its new form in one
 * layout, and the motion is then played from the old look to the new (the technique called FLIP): the glass layer's edges, the field and the send button as `transform`, and the rows that appear or go
 * as `opacity`. Nothing the animation touches changes the layout of anything else, so a key typed during the 350 ms morph costs what any key costs.
 * Interruptible: a flip that begins while one runs starts from what is on screen (the measured rectangles include the running animations), and cancels the old ones.
 */

/** What the composer looked like just before it changed: where its glass, field and button were on screen, and a copy of the rows that are about to go. */
export interface FoldSnap {
  /** The glass layer's box, with any running animation applied. */
  glass: DOMRect;
  /** The text field's and the button's boxes (the button is null when the field row has none). */
  input: DOMRect;
  send: DOMRect | null;
  /** Copies of the rows that leave (a collapse), each with where it was in the panel, to fade out. */
  ghosts: { node: HTMLElement; top: number; left: number; width: number; height: number }[];
}

/** Whether a morph may run now: a pointer or touch is driving, the person has not turned motion off and the system does not ask for less. The keyboard's change is made at once. */
export function motionAllowed(): boolean {
  const d = document.documentElement.dataset;
  return d.input !== "key" && d.motion !== "off" && !matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Animations the last flip started, cancelled when the next one begins. */
const running = new WeakMap<HTMLElement, Animation[]>();

/** The parts of the composer panel the morph moves. */
function parts(panel: HTMLElement) {
  const glass = panel.querySelector<HTMLElement>(":scope > .glass");
  const row = panel.querySelector<HTMLElement>(":scope > [data-fold-row]");
  return { glass, input: row?.firstElementChild as HTMLElement | null, send: (row && row.children.length > 1 ? row.lastElementChild : null) as HTMLElement | null };
}

/**
 * Reads the composer as it is on screen now and stops any morph in progress (so the next layout is measured clean). Call before the DOM changes. `closing` also copies the rows that are about to leave.
 * Cost: a few rectangle reads and, for a collapse, a clone of the two row elements.
 */
export function snapshot(panel: HTMLElement, closing: boolean): FoldSnap | null {
  const { glass, input, send } = parts(panel);
  if (!glass || !input) return null;
  const snap: FoldSnap = { glass: glass.getBoundingClientRect(), input: input.getBoundingClientRect(), send: send?.getBoundingClientRect() ?? null, ghosts: [] };
  const base = panel.getBoundingClientRect();
  for (const a of running.get(panel) ?? []) a.cancel();
  running.delete(panel);
  for (const g of panel.querySelectorAll(":scope > [data-fold-ghost]")) g.remove();
  if (closing) for (const row of panel.querySelectorAll<HTMLElement>(":scope > [data-fold]")) {
    const r = row.getBoundingClientRect();
    if (!r.height) continue;
    const node = row.cloneNode(true) as HTMLElement;
    node.removeAttribute("data-fold"); node.setAttribute("data-fold-ghost", ""); node.setAttribute("aria-hidden", "true"); node.setAttribute("inert", "");
    snap.ghosts.push({ node, top: r.top - base.top, left: r.left - base.left, width: r.width, height: r.height });
  }
  return snap;
}

/**
 * Plays the morph from `snap` (the old look) to the layout now in the DOM, over `ms` on `easing` (the motion tokens). Call after the DOM has changed. The glass layer's top, right, bottom and left offsets
 * run from where the old glass was to the panel's own box; the field and the button slide from where they were; rows that appear fade and rise in, rows that went fade out as copies.
 * Cost: one layout read of the panel and its parts, then compositor animations (the glass's offsets are on a leaf with no children: no other layout).
 */
export function play(panel: HTMLElement, snap: FoldSnap, ms: number, easing: string): void {
  const { glass, input, send } = parts(panel);
  if (!glass || !input) return;
  const anims: Animation[] = [];
  const opts: KeyframeAnimationOptions = { duration: ms, easing };
  const p = panel.getBoundingClientRect();
  const o = snap.glass;
  anims.push(glass.animate([{ top: `${o.top - p.top}px`, right: `${p.right - o.right}px`, bottom: `${p.bottom - o.bottom}px`, left: `${o.left - p.left}px` }, { top: "0px", right: "0px", bottom: "0px", left: "0px" }], opts));
  const slide = (el: HTMLElement, from: DOMRect) => {
    const r = el.getBoundingClientRect();
    const dx = from.left - r.left, dy = from.top - r.top;
    if (Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5) anims.push(el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }], opts));
  };
  slide(input, snap.input);
  if (send && snap.send) slide(send, snap.send);
  for (const row of panel.querySelectorAll<HTMLElement>(":scope > [data-fold]")) anims.push(row.animate([{ opacity: 0, transform: "translateY(6px)" }, { opacity: 1, transform: "none" }], opts));
  for (const g of snap.ghosts) {
    Object.assign(g.node.style, { position: "absolute", top: `${g.top}px`, left: `${g.left}px`, width: `${g.width}px`, height: `${g.height}px`, margin: "0", pointerEvents: "none" });
    panel.append(g.node);
    const fade = g.node.animate([{ opacity: 1 }, { opacity: 0 }], { duration: ms * 0.4, easing: "linear", fill: "forwards" });
    fade.finished.then(() => g.node.remove(), () => g.node.remove());
    anims.push(fade);
  }
  running.set(panel, anims);
}
