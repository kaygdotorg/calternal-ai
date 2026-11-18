/**
 * Warm tooltips, ported from the existing app (dependency-free; see docs/ui-design-system.md for the comparison with sveltebits' warm-tooltip).
 * The first tooltip waits 400 ms with nothing showing, then pops from its trigger; within 600 ms of the last one, moving to a neighbour
 * makes the same tooltip travel there on the spring while its words slide over. One shared element serves the whole page. Mouse hover,
 * keyboard focus-visible and long-press on touch show it; scroll and Escape hide it. The label is also the control's accessible name.
 * Cost: one DOM node for the page, one listener set per trigger, and one rAF loop only while a tooltip is open.
 */

import { adaptive, scheduleMaterial } from "./material";

/** Which side of its trigger a tooltip prefers. */
export type Side = "top" | "bottom" | "left" | "right";
/** What a tooltip says: the label, an optional shortcut (written the Mac way), and the preferred side. */
export type Tip = { label: string; kbd?: string; side?: Side };

const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
/** Shortcuts are written the Mac way; elsewhere they read Ctrl, Alt and Shift. */
export function keys(k: string): string {
  if (isMac) return k;
  return k.replace(/⌘/g, "Ctrl+").replace(/⌥/g, "Alt+").replace(/⇧/g, "Shift+").replace(/⌫/g, "Backspace").replace(/↵/g, "Enter").replace(/⇥/g, "Tab");
}
/** One chip per key: "⌘⇧D" becomes ⌘ ⇧ D. */
function keyChips(k: string): string[] {
  return keys(k).split(/(?<=[⌘⇧⌥⌃])|\+/).filter(Boolean).map((c) => (c === "⇥" ? "Tab" : c));
}

const DELAY = 400;
const WARM_WINDOW = 600;
const LONG_PRESS = 500;
const TRAVEL_MS = 320;
const SHIFT = 8; // px the words slide when the tooltip travels

let root: HTMLDivElement | null = null;
let current: HTMLElement | null = null;
let lastX = 0;
let lastY = 0;
let open = false;
let showTimer = 0;
let hideTimer = 0;
let warmUntil = 0;
let pressTimer = 0;
let size = { w: 0, h: 0 }; // the open tooltip's own size
let frame = 0;

const reduce = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const dataOf = (n: HTMLElement) => (n as HTMLElement & { __tip?: Tip }).__tip;

function ensure() {
  if (root) return root;
  root = document.createElement("div");
  root.className = "warm-tip";
  root.setAttribute("aria-hidden", "true"); // the trigger already carries the name
  // The surface is the shared glass (components/Glass, styles/glass.css), tinted by what is behind it like every floating surface (lib/material.ts); `dense` because it holds a line of words.
  root.innerHTML = '<div class="wt-surface glass dense"></div><div class="wt-stage"></div>';
  document.body.append(root);
  adaptive(root.querySelector<HTMLElement>(".wt-surface")!);
  return root;
}

function content(d: Tip) {
  const c = document.createElement("div");
  c.className = "wt-content";
  const label = document.createElement("span");
  label.textContent = d.label;
  c.append(label);
  if (d.kbd) {
    // One chip per key (⌘ then B), as the menus draw them; a lone key reads as a little circle.
    for (const part of keyChips(d.kbd)) {
      const k = document.createElement("kbd");
      k.textContent = part;
      if ([...part].length === 1) k.className = "single";
      c.append(k);
    }
  }
  return c;
}

function geometry(target: HTMLElement, side: Side | undefined, w: number, h: number) {
  const r = target.getBoundingClientRect();
  const gap = 8;
  let s: Side = side ?? "top";
  if (s === "top" && r.top - h - gap < 4) s = "bottom";
  if (s === "bottom" && r.bottom + h + gap > innerHeight - 4) s = "top";
  let x = r.left + r.width / 2 - w / 2;
  let y = s === "top" ? r.top - h - gap : r.bottom + gap;
  if (s === "left") [x, y] = [r.left - w - gap, r.top + r.height / 2 - h / 2];
  if (s === "right") [x, y] = [r.right + gap, r.top + r.height / 2 - h / 2];
  x = Math.max(6, Math.min(x, innerWidth - w - 6));
  const origin = { top: "50% 100%", bottom: "50% 0%", left: "100% 50%", right: "0% 50%" }[s];
  return { x: Math.round(x), y: Math.round(y), origin };
}

function show(target: HTMLElement, how: "pop" | "travel" | "instant") {
  const d = dataOf(target);
  if (!d || !d.label) return;
  const t = ensure();
  const stage = t.querySelector<HTMLDivElement>(".wt-stage")!;
  const next = content(d);
  // Measure the new words before they take the stage.
  next.style.visibility = "hidden";
  stage.append(next);
  const w = next.offsetWidth;
  const h = next.offsetHeight;
  next.style.visibility = "";
  const g = geometry(target, d.side, w, h);
  const moving = how === "travel" && open && !reduce();

  t.dataset.motion = moving ? "travel" : how === "pop" && !reduce() ? "pop" : "instant";
  t.style.width = `${w}px`;
  t.style.height = `${h}px`;
  t.style.translate = `${g.x}px ${g.y}px`;
  t.style.transformOrigin = g.origin;
  scheduleMaterial(); // it is somewhere else now: its tint is read again from what is behind it

  const old = [...stage.children].filter((c) => c !== next) as HTMLElement[];
  if (moving) {
    // Words slide in the direction of travel, with a breath of blur to bridge the swap.
    const dir = Math.sign(g.x - lastX) || 1;
    for (const o of old)
      o.animate(
        [{ opacity: 1, transform: "translateX(0)", filter: "blur(0)" }, { opacity: 0, transform: `translateX(${-dir * SHIFT}px)`, filter: "blur(3px)" }],
        { duration: 140, easing: "cubic-bezier(0.23, 1, 0.32, 1)", fill: "forwards" },
      ).onfinish = () => o.remove();
    next.animate(
      [{ opacity: 0, transform: `translateX(${dir * SHIFT}px)`, filter: "blur(3px)" }, { opacity: 1, transform: "translateX(0)", filter: "blur(0)" }],
      { duration: 220, easing: "cubic-bezier(0.23, 1, 0.32, 1)" },
    );
  } else old.forEach((o) => o.remove());

  lastX = g.x;
  lastY = g.y;
  size = { w, h };
  current = target;
  open = true;
  t.dataset.open = "true";
  clearTimeout(hideTimer);
  cancelAnimationFrame(frame);
  frame = requestAnimationFrame(stayOn);
}

/**
 * While a tooltip is open it stays centred on its trigger, even if the trigger moves under it (the
 * bar growing or shrinking, a list shifting). It was measured once, when it opened; this reads the
 * trigger each frame and moves the tooltip only if the trigger has moved.
 */
function stayOn() {
  if (!open || !current || !root) return;
  if (!current.isConnected) return hide(true);
  const g = geometry(current, dataOf(current)?.side, size.w, size.h);
  if (Math.abs(g.x - lastX) > 0.5 || Math.abs(g.y - lastY) > 0.5) {
    root.style.translate = `${g.x}px ${g.y}px`;
    root.style.transformOrigin = g.origin;
    lastX = g.x;
    lastY = g.y;
  }
  frame = requestAnimationFrame(stayOn);
}

function hide(now = false) {
  clearTimeout(showTimer);
  cancelAnimationFrame(frame);
  if (!root || !open) return;
  warmUntil = Date.now() + WARM_WINDOW;
  root.dataset.motion = now || reduce() ? "instant" : "pop";
  root.dataset.open = "false";
  open = false;
  current = null;
}

function tipEngine(node: HTMLElement, data: Tip) {
  const apply = (d: Tip) => {
    (node as HTMLElement & { __tip?: Tip }).__tip = d;
    if (current === node) show(node, "instant");
  };
  apply(data);

  const enter = (e: PointerEvent) => {
    if (e.pointerType !== "mouse") return;
    clearTimeout(showTimer);
    if (open || Date.now() < warmUntil) show(node, open ? "travel" : "instant");
    else {
      showTimer = window.setTimeout(() => show(node, "pop"), DELAY);
    }
  };
  const leave = () => {
    clearTimeout(showTimer);
    if (current === node) hide();
  };
  const down = (e: PointerEvent) => {
    if (e.pointerType === "mouse") {
      hide(true);
      warmUntil = 0;
      return;
    }
    pressTimer = window.setTimeout(() => show(node, "pop"), LONG_PRESS);
  };
  const up = () => {
    clearTimeout(pressTimer);
    if (current === node) hideTimer = window.setTimeout(() => hide(), 1200);
  };
  const focus = () => {
    if (node.matches(":focus-visible") && !node.hasAttribute("data-tip-quiet")) show(node, open ? "travel" : Date.now() < warmUntil ? "instant" : "pop");
  };
  node.addEventListener("pointerenter", enter);
  node.addEventListener("pointerleave", leave);
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  node.addEventListener("focus", focus);
  node.addEventListener("blur", leave);
  return {
    update: apply,
    destroy() {
      if (current === node) hide(true);
      node.removeEventListener("pointerenter", enter);
      node.removeEventListener("pointerleave", leave);
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", up);
      node.removeEventListener("focus", focus);
      node.removeEventListener("blur", leave);
    },
  };
}

addEventListener("scroll", () => hide(true), true);
addEventListener("keydown", (e) => e.key === "Escape" && hide(true));

/** Svelte action: `use:tip={{ label: "Bold", kbd: "⌘B" }}`. Sets the control's accessible name at once; the tooltip shows on hover, focus-visible or long-press. */
export function tip(node: HTMLElement, data: Tip) {
  const name = (d: Tip) => {
    if (!d.label) return;
    const text = node.textContent?.trim() ?? "";
    if (!text || node.dataset.iconOnly !== undefined || d.label.toLowerCase().startsWith(text.toLowerCase())) node.setAttribute("aria-label", d.label);
  };
  name(data);
  const live = tipEngine(node, data);
  return { update(d: Tip) { name(d); live.update(d); }, destroy: () => live.destroy() };
}
