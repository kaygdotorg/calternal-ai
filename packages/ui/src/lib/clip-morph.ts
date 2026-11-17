/**
 * The button ⇄ menu morph of a popover that grows out of the control that opened it and shrinks back into it, the way Apple's native ⋯ overflow menus do.
 * Ported from the owner's `calternal-ui` plugin (`frame/clip-morph.ts`, itself fitted from iOS 27's Files and Reminders at 60 fps, 2026-11-15); the existing app's
 * `growFromRect`/`shrinkToRect` (mockup `lib/morph-glass.ts`) is the same idea on a fixed curve and is superseded by this fit.
 *
 *   Open   The button's own shape grows into the menu on a damped spring (response 330 ms, damping 0.74: a 2 to 4 % overshoot peaking near 290 ms, settled by
 *          480 ms), staying a full capsule until 75 % grown and then tightening to the menu's radius. The rows are laid out at their final size, clipped by the
 *          shape and blurred, and come into focus (blur 10 px to 0, opacity 0 to 1) between 75 % and 95 % grown. The overshoot shows as a scale about the button's corner.
 *   Close  The rows blur and fade out first (70 ms, linear); the empty glass shrinks into the button's rectangle as a capsule over 250 ms on
 *          cubic-bezier(0.2, 0, 0.5, 1); at 160 ms the caller brings the button back (`onReturn`).
 *
 * Why a requestAnimationFrame loop with inline styles and not `element.animate({ clipPath })`: Chrome runs clip-path animations on the compositor and the hand-offs
 * flicker (an unclipped "empty panel" for about two frames at the start, a blank layer when the finished animation is cancelled). Here the first frame is set
 * synchronously, before the browser paints, and the end removes the inline styles. A menu is small: one clip and a few opacities per frame.
 *
 * What is clipped. The clip goes on the glass layer (the element that owns the `backdrop-filter`, so its blur is clipped with it) and on the words, never on an
 * ancestor of the glass: an ancestor with a clip-path becomes a backdrop root and the blur would sample nothing until the clip is removed (a visible pop at the end).
 * Interruptible: a new morph starts from the progress the running one had reached, so reversing half way is continuous.
 * Keyboard use and reduced motion do not morph (the caller checks `morphAllowed`).
 * Cost: per frame, two `clip-path` writes, one `scale` write and an opacity and filter write per child of the words; 250 to 480 ms per open or close.
 */

/** Emil Kowalski's strong ease-out, cubic-bezier(0.23, 1, 0.32, 1), as CSS gives it. */
export const EASE_OUT_PTS = [0.23, 1, 0.32, 1] as const;

/** A CSS cubic-bezier(x1, y1, x2, y2) as a function of progress 0..1 (Newton, then bisection), so the curve matches CSS's exactly. */
export function bezier(x1: number, y1: number, x2: number, y2: number): (t: number) => number {
  const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
  const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
  const x = (u: number) => ((ax * u + bx) * u + cx) * u;
  const y = (u: number) => ((ay * u + by) * u + cy) * u;
  const dx = (u: number) => (3 * ax * u + 2 * bx) * u + cx;
  return (t) => {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    let u = t;
    for (let i = 0; i < 6; i++) {
      const err = x(u) - t, d = dx(u);
      if (Math.abs(err) < 1e-5) return y(u);
      if (Math.abs(d) < 1e-6) break;
      u -= err / d;
    }
    let lo = 0, hi = 1; u = t;
    for (let i = 0; i < 24; i++) { if (x(u) < t) lo = u; else hi = u; u = (lo + hi) / 2; }
    return y(u);
  };
}

/** The pieces of a popover the morph moves. */
export interface MorphParts {
  /** The whole floating box: scaled about the button's corner for the overshoot. */
  box: HTMLElement;
  /** The glass layer (owns the backdrop blur): clipped. */
  glass: HTMLElement;
  /** The words wrapper: clipped, and each child fades and unblurs. */
  words: HTMLElement;
  /** The control the popover grows from and returns to. */
  from: HTMLElement;
}

/** Extras for the morph. */
export interface MorphOptions {
  /** Closing: called when the button should reappear (160 ms in, as iOS brings it back before the glass has fully shrunk into it). */
  onReturn?: () => void;
}

const OPEN_SETTLE = 480, CLOSE_MS = 250, RETURN_AT = 160, ROWS_OUT = 70;
const closeEase = bezier(0.2, 0, 0.5, 1);
const outEase = bezier(...EASE_OUT_PTS);

/** Whether a morph may run now: a pointer or touch is driving, motion is not reduced. (Keyboard use applies end states at once.) */
export function morphAllowed(): boolean {
  return document.documentElement.dataset.input !== "key" && !matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** How far the running morph on a box has got, 0 (the button) to 1 (the menu), and its stop function. */
const running = new WeakMap<HTMLElement, { stop: () => void; g: () => number }>();

/** Stops a running morph on `box` and drops every inline style it set. Returns the progress it had reached (1 when none was running). */
export function stopMorph(box: HTMLElement): number {
  const r = running.get(box);
  const g = r ? r.g() : 1;
  r?.stop();
  return g;
}

/** A damped spring's progress (0 to 1, overshooting a little): `response` is the period in ms, `damping` the ratio. */
function spring(t: number, response = 330, damping = 0.74): number {
  if (t <= 0) return 0;
  const w = (2 * Math.PI) / response;
  const wd = w * Math.sqrt(1 - damping * damping);
  return 1 - Math.exp(-damping * w * t) * (Math.cos(wd * t) + ((damping * w) / wd) * Math.sin(wd * t));
}

/** The button coming back after a close, as iOS's: from about half size, unblurring, a slight swell. */
export function buttonSwell(el: HTMLElement, from = 0.9): void {
  el.animate?.(
    { scale: [from, 1 + (1 - from) * 0.08, 1], filter: ["blur(3px)", "blur(0px)", "blur(0px)"], opacity: [0.4, 1, 1] },
    { duration: 260, easing: "cubic-bezier(0.23, 1, 0.32, 1)" },
  );
}

/**
 * Runs the morph of `parts` in one direction, from wherever the previous one stopped. Resolves when it ends (opening leaves no inline styles; closing leaves the
 * glass held at the button's shape, and the caller then unmounts the popover). The first frame is painted synchronously.
 */
export function clipMorph(parts: MorphParts, opening: boolean, opts: MorphOptions = {}): Promise<void> {
  const { box, glass, words, from } = parts;
  const g0 = running.has(box) ? stopMorph(box) : opening ? 0 : 1;
  const kids = [...words.children] as HTMLElement[];
  const m = box.getBoundingClientRect();
  const p = from.getBoundingClientRect();
  const r1 = parseFloat(getComputedStyle(glass).borderTopLeftRadius) || 14;
  // The button's rectangle inside the menu's box, as insets from each side (top, right, bottom, left).
  const small = [p.top - m.top, m.right - p.right, m.bottom - p.bottom, p.left - m.left];
  // Overshoot about the button's corner that the menu shares: the bottom corner when the menu grows upward, the top when downward; the left or the right likewise.
  const originX = small[3] <= small[1] ? p.left - m.left : p.right - m.left;
  const originY = small[0] >= small[2] ? p.bottom - m.top : p.top - m.top;

  let raf = 0, g = g0;
  let resolve!: () => void;
  const done = new Promise<void>((r) => (resolve = r));
  const clear = () => {
    for (const el of [glass, words]) el.style.removeProperty("clip-path");
    box.style.removeProperty("scale");
    box.style.removeProperty("transform-origin");
    for (const k of kids) { k.style.removeProperty("opacity"); k.style.removeProperty("filter"); }
  };
  const stop = () => { cancelAnimationFrame(raf); running.delete(box); clear(); resolve(); };
  running.set(box, { stop, g: () => g });
  box.style.setProperty("transform-origin", `${originX.toFixed(1)}px ${originY.toFixed(1)}px`);

  const clipAt = (k0: number, round: boolean): string => {
    const k = Math.max(0, Math.min(1, k0));
    const ins = small.map((v) => v * (1 - k));
    const w = m.width - ins[1] - ins[3];
    const h = m.height - ins[0] - ins[2];
    const capsule = Math.max(0, Math.min(w, h) / 2);
    const corner = round ? capsule : k < 0.75 ? capsule : capsule + (Math.min(r1, capsule) - capsule) * ((k - 0.75) / 0.25);
    return `inset(${ins[0].toFixed(2)}px ${ins[1].toFixed(2)}px ${ins[2].toFixed(2)}px ${ins[3].toFixed(2)}px round ${corner.toFixed(2)}px)`;
  };
  const setClip = (c: string) => { glass.style.setProperty("clip-path", c); words.style.setProperty("clip-path", c); };
  const rows = (opacity: number, blur: number) => {
    const o = opacity.toFixed(3), f = blur > 0.05 ? `blur(${blur.toFixed(2)}px)` : "none";
    for (const k of kids) { k.style.setProperty("opacity", o); k.style.setProperty("filter", f); }
  };
  const focusOf = (k: number) => Math.max(0, Math.min(1, (Math.min(k, 1) - 0.75) / 0.2));

  const t0 = performance.now();
  let returned = false;
  const paint = (now: number) => {
    const t = now - t0;
    if (opening) {
      // From the button: the spring. From part way (reversing a close): the strong ease-out the rest of the way, so the hand-off has no jump.
      g = g0 < 0.01 ? spring(t) : g0 + (1 - g0) * outEase(Math.min(1, t / (OPEN_SETTLE * 0.6)));
      setClip(clipAt(g, false));
      box.style.setProperty("scale", g > 1 ? (1 + (g - 1) * 0.5).toFixed(4) : "1");
      const f = focusOf(g);
      rows(f, 10 * (1 - f));
      if (t >= (g0 < 0.01 ? OPEN_SETTLE : OPEN_SETTLE * 0.6)) { running.delete(box); clear(); resolve(); return; }
    } else {
      g = Math.min(1, g0) * (1 - closeEase(Math.min(1, t / CLOSE_MS)));
      setClip(clipAt(g, t > 30));
      const f0 = focusOf(g0), fade = Math.min(1, t / ROWS_OUT);
      rows(f0 * (1 - fade), 8 * fade);
      box.style.setProperty("scale", "1");
      if (!returned && t >= RETURN_AT) { returned = true; opts.onReturn?.(); }
      if (t >= CLOSE_MS) { running.delete(box); if (!returned) opts.onReturn?.(); resolve(); return; }
    }
    raf = requestAnimationFrame(paint);
  };
  paint(t0);
  return done;
}
