// Ported from the existing app (apps/web/src/rows/swipe-row.ts on origin/dev, #400): same thresholds, springs and look; the only changes are the import paths and the right-edge guard below.
// One row's swipe (#400): its position, its spring, its buttons' look. The page's lists never give
// a row its own listeners; swipe-rows.ts owns the events and hands each one to the row it landed on,
// created the first time that row is touched (`createRow`).
//
// How it feels:
// - The buttons stand still where they are, one pill each with its label under it and a gap between
//   them; the row slides off them. The outer one (the one a long swipe runs) shows first, the next
//   appears beside it as the row goes further, each fading and growing in as it is uncovered. They
//   never overlap.
// - On a desktop a two-finger sideways scroll (trackpad or Magic Mouse, wheel events with deltaX)
//   moves the row the same way: it follows, and a spring settles it open or shut when the scroll
//   ends. The axis is decided by the first event of a gesture; vertical scrolls scroll the list.
// - The row follows the finger 1:1 until its buttons show, then stretches with resistance.
// - Past the buttons the outer pill stretches elastically with the finger: its width follows the
//   pull continuously (a smooth ramp, no jump), taking over the gap and turning solid, while the
//   others slide under the row, shrink and fade. Crossing the commit point only gives a small
//   tick (a scale beat), never a jump.
// - Let go and a spring takes it open or shut, keeping the finger's speed: open when past half the
//   buttons, or flicked faster than 0.3 px/ms. A long swipe runs the outer button instead.
// - One row is open at a time (swipe-open.ts).
//
// When it never starts (kayg 11-07): with two or more fingers down (a second finger landing
// mid-swipe puts the row back, it never lets go as a swipe), while the list is being selected
// (`handle.off()`: the bottom bar holds the actions), and when selecting begins mid-swipe (the
// next move or the let-go puts the row back). Putting a row back never runs its action.
//
// When it starts: the rule shared with block swipe-select (#397), in `@calternal/editor/swipe-start`:
// sideways must clearly win over vertical (twice as far), so a mostly-vertical drag is a scroll.
// Touch uses Touch Events with non-passive listeners: on iOS a row that waits for Pointer Events
// loses the touch to scrolling (WebKit 232545). From the claiming move on, every move is ours
// (preventDefault), as in #397. If the phone has already begun scrolling (the move can't be
// cancelled), the row lets the scroll have it rather than both moving at once.
//
// Every frame writes transforms, opacity and the outer pill's width, inside a tray that is its own
// layout box. The one layout read is the row's width, once per touch.
import { BACK_EDGE, CLAIM, QUICK, claimsSwipe } from "./start";
import { springTo } from "./spring";
import { FLICK, GAP, fullAt, band, sideOf, tileLook, trayWidth, type Side, type SideConf } from "./math";
import { markOpen, markShut, shutOthers } from "./open";

export type { Side, SideConf };
/** What a swipe row is made of: the buttons on each side and what to do when a long swipe is let go. Read fresh on each touch, so it may change between swipes. */
export type SwipeConfig = {
  /** Buttons on each side, and what a long swipe does there. */
  lead: SideConf;
  trail: SideConf;
  /** A long swipe was let go: run the outer button of that side. A promise holds a leaving row. */
  onfull: (side: Side) => void | Promise<unknown>;
  /** A hold on the row has begun to matter (it lifted, its menu opened): this row never claims it as a swipe. */
  busy?: () => boolean;
};
/** What a row tells the list about itself. */
export type RowHandle = {
  config: () => SwipeConfig;
  /** The first touch: draw the buttons now, before they are needed. */
  ontouch: () => void;
  /** The list is being selected right now: this row doesn't swipe (cheap to ask, unlike `config`). */
  off: () => boolean;
};

const reduce = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const isIOS = /iPhone|iPad|iPod/.test(navigator.userAgent) || (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
const WHEEL_IDLE = 120; // the wheel is quiet this long: the gesture is over (inertia included)

type Sample = { x: number; t: number };
type Gesture = { x0: number; y0: number; t0: number; base: number; state: "wait" | "swipe" | "done"; moved: boolean; samples: Sample[] };
type WheelGesture = { state: "swipe" | "other"; raw: number; samples: Sample[]; timer: number };

function speed(samples: Sample[]) {
  const a = samples[0];
  const b = samples[samples.length - 1];
  return a && b && b.t > a.t ? (b.x - a.x) / (b.t - a.t) : 0;
}

/**
 * Attaches the swipe behaviour to one row element: touch and wheel tracking, the spring on release and the open-row bookkeeping. Cost: no work while idle;
 * during a drag one transform write per frame and the row's width read once per touch. Returns the touch, wheel and key handlers the component wires to the row and a `destroy` that stops the animation and forgets the row.
 */
export function createRow(node: HTMLElement, handle: RowHandle) {
  const cfg = () => handle.config();
  const content = () => node.querySelector<HTMLElement>(":scope > .ra-content")!;
  let cache: Record<string, HTMLElement[]> = {};
  const looks = new WeakMap<HTMLElement, string>();
  const tiles = (side: Side) => (cache[side] = cache[side]?.length ? cache[side] : [...node.querySelectorAll<HTMLElement>(`.ra-side.${side > 0 ? "lead" : "trail"} .ra-act`)]);

  // The buttons' tray is cut to the part of the row the card has uncovered, so the card itself can
  // stay see-through, exactly as the row looks at rest (its own tint over the list's glass).
  let clipKey = "";
  const trayEl = () => node.querySelector<HTMLElement>(":scope > .ra-tray");
  /** The tray's clip at row offset `d`: the strip from the card's moving edge to the row's edge. The
   * tray sits GAP inside the row on every side. */
  const clipAt = (d: number, s: Side) =>
    d === 0 ? (s < 0 ? "inset(0 0 0 100%)" : "inset(0 100% 0 0)") : d < 0 ? `inset(0 0 0 ${Math.max(0, width + d - GAP)}px)` : `inset(0 ${Math.max(0, width - GAP - d)}px 0 0)`;

  // Where the row is: `x` is the content's offset (+ shows the left buttons), `rest` where it settles.
  let x = 0;
  let rest = 0;
  let width = 360;
  let armed = false;
  let raf = 0;
  let anims: Animation[] = [];
  let g: Gesture | null = null;
  let wh: WheelGesture | null = null;

  const conf = (s: Side) => (s > 0 ? cfg().lead : cfg().trail);
  const W = (s: Side) => trayWidth(conf(s));
  const swiping = () => g?.state === "swipe" || wh?.state === "swipe";

  // ---- Painting

  function paint(d: number) {
    x = d;
    content().style.transform = d ? `translate3d(${d}px,0,0)` : "";
    const tray = trayEl();
    if (tray) {
      const clip = clipAt(d, sideOf(d));
      if (clip !== clipKey) tray.style.clipPath = clipKey = clip;
    }
    const s = sideOf(d);
    // Only when it changes: a write, even of the same value, restyles the row.
    const side = d === 0 ? "" : s > 0 ? "lead" : "trail";
    if (node.dataset.side !== side) node.dataset.side = side;
    const ts = tiles(s);
    if (!ts.length) return;
    const a = Math.abs(d);
    const c = conf(s);
    ts.forEach((t, i) => {
      const st = tileLook(i, s, c, width, a);
      // Only what changed: a pill that is still (fully out, or not yet) costs nothing.
      const key = `${st.opacity}|${st.scale}|${st.width}|${st.solid}|${st.shift}`;
      if (looks.get(t) === key) return;
      looks.set(t, key);
      t.style.opacity = String(st.opacity);
      t.style.scale = String(st.scale);
      if (i === 0) {
        t.style.width = `${st.width}px`;
        t.style.setProperty("--s", String(st.solid));
      } else t.style.transform = st.shift ? `translate3d(${st.shift}px,0,0)` : "";
    });
  }

  const schedule = () => (raf ||= requestAnimationFrame(frame));
  function frame() {
    raf = 0;
    paint(x);
    if (swiping()) schedule();
  }

  /** The outer pill of side `s` swells to `peak` and back in `duration` ms: the commit tick and the let-go pulse. */
  function beat(s: Side, peak: number, duration: number) {
    if (!reduce()) tiles(s)[0]?.animate([{ transform: "scale(1)" }, { transform: `scale(${peak})` }, { transform: "scale(1)" }], { duration, easing: "cubic-bezier(0.3, 0, 0.2, 1)" });
  }

  function setArmed(on: boolean) {
    if (on === armed) return;
    armed = on;
    node.classList.toggle("ra-armed", on);
    // A light tick at the commit point: the outer pill gives one small beat (no jump, it was
    // already taking over).
    if (on) beat(sideOf(x), 1.06, 260);
    // Android can tick; iOS Safari can't from a moving finger, so the detent is visual only.
    if (on && !isIOS && "vibrate" in navigator) navigator.vibrate(8);
  }

  // ---- Settling (WAAPI, keeps the finger's speed)

  function stopAnims() {
    if (!anims.length) return;
    // Pick the row up where it is. One style read, at the start of a touch, never per frame.
    const m = new DOMMatrixReadOnly(getComputedStyle(content()).transform);
    for (const a of anims) a.cancel();
    anims = [];
    for (const t of Object.values(cache).flat()) looks.delete(t);
    paint(m.m41);
  }

  function settle(to: number, velocity: number, done?: () => void) {
    const from = x;
    const s = sideOf(from || to);
    const ts = tiles(s);
    const c = conf(s);
    const finish = () => {
      for (const a of anims) a.cancel();
      anims = [];
      for (const t of ts) looks.delete(t);
      paint(to);
      if (to === 0) node.dataset.side = "";
      done?.();
    };
    if (reduce() || Math.abs(to - from) < 0.5) return finish();
    for (const t of ts) looks.delete(t);
    const { easing, duration } = springTo(from, to, velocity);
    const opt: KeyframeAnimationOptions = { duration, easing, fill: "forwards" };
    for (const a of anims) a.cancel();
    anims = [];
    anims.push(content().animate([{ transform: `translate3d(${from}px,0,0)` }, { transform: `translate3d(${to}px,0,0)` }], opt));
    // The tray's cut follows the card edge. Its side is the swipe's, so it shows whichever way it goes.
    const tray = trayEl();
    if (tray) {
      const side = sideOf(from || to);
      anims.push(tray.animate([{ clipPath: clipAt(from, side) }, { clipPath: clipAt(to, side) }], opt));
      clipKey = clipAt(to, side);
      tray.style.clipPath = clipKey;
    }
    // The pills fade, grow, shrink and slide with the row; the outer one's solid colour fades by CSS.
    ts.forEach((t, i) => {
      const A = tileLook(i, s, c, width, Math.abs(from));
      const B = tileLook(i, s, c, width, Math.abs(to));
      const kf = (st: typeof A) => ({
        opacity: String(st.opacity),
        scale: String(st.scale),
        ...(i === 0 ? { width: `${st.width}px` } : { transform: `translate3d(${st.shift}px,0,0)` }),
      });
      anims.push(t.animate([kf(A), kf(B)], opt));
      if (i === 0) t.style.setProperty("--s", String(B.solid));
    });
    x = to;
    anims[0].finished.then(finish, () => {});
  }

  function shut(animate = true) {
    if (swiping()) return;
    setArmed(false);
    forget();
    if (animate && x !== 0) settle(0, 0);
    else {
      for (const a of anims) a.cancel();
      anims = [];
      paint(0);
    }
  }

  function opened(side: Side) {
    rest = side * W(side);
    node.classList.add("ra-open");
    shutOthers(node);
    markOpen({ node, shut: (animate = true) => shut(animate && !reduce()) });
  }

  function forget() {
    rest = 0;
    node.classList.remove("ra-open");
    markShut(node);
  }


  /** Let go at speed `v` (px/ms). */
  function release(v: number) {
    const s = sideOf(x);
    const c = conf(s);
    if (armed && c.full) {
      forget();
      if (c.leaves) {
        // It goes: out past the edge, then the list closes the gap once the action is done. If it
        // stays (the action failed), it comes back.
        settle(s * width, v, () => {
          const back = () => requestAnimationFrame(() => node.isConnected && (setArmed(false), shut(false)));
          Promise.resolve(cfg().onfull(s)).then(back, back);
        });
      } else {
        // It stays: the pill pulses once as the row springs back, and the action says what it did.
        beat(s, 1.12, 420); // the outer pill, taken over the gap, gives one beat as the row springs back
        void cfg().onfull(s);
        settle(0, v, () => setArmed(false));
      }
      return;
    }
    setArmed(false);
    const a = Math.abs(x);
    const toward = Math.sign(v) === s;
    const flick = Math.abs(v) > FLICK;
    const wide = W(s) > 0 && a > 4 && (flick ? toward : a > W(s) / 2);
    if (wide) {
      opened(s);
      settle(s * W(s), v);
    } else {
      forget();
      settle(0, v);
    }
  }

  /** The finger (or wheel) is at `raw` px from where the row began: move to it. */
  function follow(raw: number, samples: Sample[], now: number) {
    samples.push({ x: raw, t: now });
    while (samples.length > 2 && now - samples[0].t > 80) samples.shift();
    const c = conf(sideOf(raw));
    setArmed(c.full && c.n > 0 && Math.abs(raw) >= fullAt(c, width));
    x = band(raw, c);
    schedule();
  }

  /** The swipe's finger or wheel is done: stop the frame loop and draw the row where it is. */
  function endSwipe() {
    node.classList.remove("ra-swiping");
    cancelAnimationFrame(raf);
    raf = 0;
    paint(x);
  }

  function finishGesture(samples: Sample[]) {
    endSwipe();
    release(speed(samples));
  }

  /**
   * Puts a swipe in progress back where it began and drops the gesture, running no action (not
   * even a long swipe's). For a second finger, a cancelled touch, or selecting starting.
   */
  function abort() {
    const moving = swiping();
    g = null;
    clearTimeout(wh?.timer);
    wh = null;
    if (!moving) return;
    endSwipe();
    setArmed(false);
    settle(rest, 0);
  }

  // ---- Touch

  function tstart(e: TouchEvent) {
    const t = e.touches[0];
    abort();
    // Never from the screen's left 24 px (the phone's swipe back) nor its right 24 px (the side panel's edge pull): those are the shell's.
    if (e.touches.length !== 1 || t.clientX <= BACK_EDGE || t.clientX >= innerWidth - BACK_EDGE || handle.off()) return;
    if ((e.target as Element).closest(".ra-tray")) return; // a button: just a tap
    width = node.getBoundingClientRect().width;
    cache = {};
    handle.ontouch();
    stopAnims();
    g = { x0: t.clientX, y0: t.clientY, t0: e.timeStamp, base: rest, state: "wait", moved: false, samples: [] };
  }

  function tmove(e: TouchEvent) {
    if (!g) return;
    // A second finger (its touchstart may have landed outside this list), or selecting began: not a swipe any more.
    if (e.touches.length !== 1 || handle.off()) return abort();
    const t = e.touches[0];
    const dx = t.clientX - g.x0;
    const dy = t.clientY - g.y0;
    const ms = e.timeStamp - g.t0;
    if (Math.hypot(dx, dy) > 8) g.moved = true;
    if (g.state === "wait" && cfg().busy?.()) {
      g.state = "done";
      return;
    }
    if (g.state === "wait") {
      if (claimsSwipe(dx, dy, ms) && e.cancelable) {
        g.state = "swipe";
        shutOthers(node);
        node.classList.add("ra-swiping");
        schedule();
      } else if (Math.abs(dy) >= CLAIM || Math.abs(dx) >= CLAIM || (ms >= QUICK && g.moved)) {
        // A scroll, a slow drag or a diagonal: not ours. An open row shuts.
        g.state = "done";
        if (rest) shut();
        return;
      } else return;
    }
    if (g.state !== "swipe") return;
    follow(g.base + dx, g.samples, e.timeStamp);
    if (e.cancelable) e.preventDefault();
  }

  function tend(e: TouchEvent) {
    const cur = g;
    g = null;
    const wasOpen = rest !== 0 && cur?.state === "wait" && !cur.moved;
    const mine = cur?.state === "swipe";
    // Selecting began while the finger was down: the row goes back, even from a long swipe.
    if (mine) {
      if (handle.off()) (g = cur), abort();
      else finishGesture(cur!.samples);
    }
    // A swipe is never also a tap; a tap on an open row only shuts it.
    if ((mine || wasOpen) && e.cancelable) e.preventDefault();
    if (wasOpen) shut();
  }


  // ---- Wheel: a two-finger sideways scroll on a trackpad or Magic Mouse moves the row like a finger.
  // The first event of a gesture decides the axis; sideways ours, vertical the list's. The gesture
  // ends when the wheel has been quiet for a moment (inertia included), and a spring settles the row.

  function wheelEnd() {
    const cur = wh;
    wh = null;
    if (cur?.state === "swipe") finishGesture(cur.samples);
  }

  function wheel(e: WheelEvent) {
    if (e.ctrlKey || handle.off() || g) return;
    const k = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? width : 1;
    const dx = e.deltaX * k;
    const dy = e.deltaY * k;
    if (!wh) {
      if (!dx && !dy) return;
      const horizontal = Math.abs(dx) > Math.abs(dy);
      wh = { state: horizontal ? "swipe" : "other", raw: rest, samples: [], timer: 0 };
      if (horizontal) {
        width = node.getBoundingClientRect().width;
        cache = {};
        handle.ontouch();
        stopAnims();
        shutOthers(node);
        node.classList.add("ra-swiping");
        schedule();
      }
    }
    clearTimeout(wh.timer);
    // `data-wheel-idle` on the root lengthens it (a scripted wheel is slow between events).
    wh.timer = window.setTimeout(wheelEnd, Number(document.documentElement.dataset.wheelIdle) || WHEEL_IDLE);
    if (wh.state !== "swipe") return;
    e.preventDefault();
    wh.raw -= dx; // fingers going left give a positive deltaX: the row goes left
    follow(wh.raw, wh.samples, e.timeStamp);
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && rest) {
      e.stopPropagation();
      shut(false);
    }
  }

  return {
    tstart,
    tmove,
    tend,
    tcancel: abort, // the system took the touch (a call, a gesture): the row goes back, nothing runs
    wheel,
    key,
    destroy() {
      cancelAnimationFrame(raf);
      clearTimeout(wh?.timer);
      markShut(node); // (a no-op unless this row is the open one)
    },
  };
}
/** What `createRow` returns: the event handlers to attach and `destroy`. */
export type Row = ReturnType<typeof createRow>;
