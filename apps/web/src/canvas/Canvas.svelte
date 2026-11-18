<script lang="ts">
  // The canvas (docs/ui-shell-status.md, "The rules"): inside the middle card, niri's structured layout of columns of windows on an endless horizontal strip. Behind `?canvas=1` in the fixtures build only.
  //
  // Panning is compositor-only: the strip is one element moved by `transform: translate3d`, written straight to its style in an animation frame, no reactive state and no layout per frame. A column is mounted a viewport before it can be seen and unmounted
  // only two viewports away (layout.ts `nearColumns`), so windows off screen are not in the DOM at all: not laid out, not painted, not styled. The wheel rule: a wheel over the focused window (or a locked one) scrolls inside it; over an unfocused window or the gaps it pans the
  // canvas; a horizontal wheel over the focused window pans the canvas when nothing under it can scroll that way. While a window holds the input lock the canvas never moves. Touch: two fingers pan the canvas, one finger scrolls the window under it and a horizontal
  // swipe is the shell's sidebar pull. Pointer-initiated adds, closes, width presets and maximise animate on the motion tokens, interruptibly, by FLIP (transform only); the same actions from the keyboard are instant.
  import { onMount, tick, untrack } from "svelte";
  import { currentScheme, Menu, onScheme, toast, type MenuItem } from "@calternal/ui";
  import { appFor } from "../apps/registry";
  import { store } from "../apps/threads/store.svelte";
  import { notes } from "../apps/notes/store.svelte";
  import { runThreadItem, threadItems } from "../apps/threads/thread-menu";
  import { href, navigate, route, segments } from "../lib/route.svelte";
  import { ariaKeys, show as showChord, shortcuts } from "../lib/shortcuts.svelte";
  import { onWallpaperColours, wallpaperExtremes } from "../appearance/wallpaper";
  import { spaces } from "../shell/spaces.svelte";
  import { shell } from "../shell/shell.svelte";
  import { centring, edgeOf, GAP, leftOf, maxScroll, nearColumns, NARROW, nextPreset, PRESETS, proportionFor, proportionOf, scrollTo, stripWidth, widthsOf, type Look } from "./layout";
  import { canvas } from "./model.svelte";
  import { frostDensity } from "./frost";
  import { hueOf, lightnessOf, windowColours } from "./surface";
  import NotePreview from "./NotePreview.svelte";
  import WindowFrame, { type WinAction } from "./WindowFrame.svelte";
  import type { IconName } from "@calternal/ui";

  let el = $state<HTMLElement>(), strip = $state<HTMLElement>();
  let vw = $state(1000), vh = $state(700);
  const width = () => el?.clientWidth || document.querySelector<HTMLElement>(".mid")?.clientWidth || vw;
  /** The scroll position now (px); `vx` is where the mounted set was last worked out, a reactive copy updated when the scroll has moved by a quarter viewport. */
  let x = $state(canvas.x), vx = $state(canvas.x);
  let frame = 0, tween = 0;
  let fadeL = $state<HTMLElement>(), fadeR = $state<HTMLElement>();

  // ---- the layout, from the rules ----
  const widths = $derived(widthsOf(canvas.cols, vw));
  const pad = $derived(centring(widths, vw));
  const narrow = $derived(vw < NARROW);

  // ---- colours: the wallpaper's hue and tone, the scheme, and the frost's density ----
  let scheme = $state(currentScheme());
  const accentHue = () => hueOf(getComputedStyle(document.documentElement).getPropertyValue("--accent-base").trim() || "#808080");
  let hue = $state<number | null>(accentHue());
  let tone = $state<{ lo: [number, number, number]; hi: [number, number, number] } | null>(null);
  onMount(() => {
    const read = () => { tone = wallpaperExtremes(); };
    read();
    const a = onScheme((s) => { scheme = s; queueMicrotask(read); });
    const b = onWallpaperColours((c) => { hue = c?.hue?.h ?? accentHue(); read(); });
    return () => { a(); b(); };
  });
  const colours = $derived(windowColours(scheme, hue, tone ? lightnessOf([0, 1, 2].map((i) => (tone!.lo[i]! + tone!.hi[i]!) / 2) as [number, number, number]) : undefined));
  /** The scheme's own paper and inks as the page resolves them (the frosted window is the page's paper over glass, with the page's inks). */
  const page = $derived.by(() => {
    void scheme;
    const probe = document.createElement("i");
    probe.style.cssText = "position:absolute;visibility:hidden;width:0;height:0";
    document.body.append(probe);
    const at = (c: string, prop: "color" | "backgroundColor") => { probe.style.color = probe.style.backgroundColor = ""; probe.style[prop] = c; const m = getComputedStyle(probe)[prop].match(/[\d.]+/g)!.slice(0, 3).map(Number); return m as [number, number, number]; };
    const out = { paper: at("var(--paper)", "backgroundColor"), ink: at("var(--ink)", "color"), ink3: at("var(--ink-3)", "color") };
    probe.remove();
    return out;
  });
  const density = $derived(tone ? frostDensity(page.paper, page.ink, page.ink3, tone.lo, tone.hi) : 0.5);

  // ---- the strip ----
  /** The edge fade: how far a window is cut off at the left and right edge of the viewport, as the complement of the edge strip's opacity (1: no fade). It is 1 at both edges unless a window crosses that edge and falls to 0 over `FADE` px of crossing. */
  const FADE = 24;
  function fades(): [number, number] {
    let l = 1, r = 1, left = edgeOf(vw) + pad;
    for (const w of widths) {
      const right = left + w;
      if (left < x && right > x) l = Math.min(l, 1 - Math.min(1, (x - left) / FADE));
      if (right > x + vw && left < x + vw) r = Math.min(r, 1 - Math.min(1, (right - (x + vw)) / FADE));
      left = right + GAP;
      if (left > x + 2 * vw) break;
    }
    return [l, r];
  }
  let fl = 1, fr = 1;
  // Frosted windows show a blurred picture of the wallpaper that must stay still while the window moves: each picture is moved against the strip's movement, in the frame that moves the strip (transform only, no layout, no style invalidation of the windows'
  // contents). Its offsets are worked out when the layout changes (`refreshPics`), from layout offsets that no animation disturbs.
  let pics: { el: HTMLElement; cl: number; oy: number }[] = [];
  let canvasLeft = 0;
  const place = () => { for (const p of pics) p.el.style.transform = `translate3d(${x - canvasLeft - p.cl}px, ${p.oy}px, 0)`; };
  function refreshPics() {
    if (!el) return;
    const cr = el.getBoundingClientRect();
    canvasLeft = cr.left;
    pics = [...el.querySelectorAll<HTMLElement>(".pic")].map((p) => {
      const slot = p.closest<HTMLElement>("[data-win]")!.parentElement as HTMLElement, col = slot.parentElement as HTMLElement;
      p.style.width = `${innerWidth}px`; p.style.height = `${innerHeight}px`;
      return { el: p, cl: Number(col.dataset.l ?? 0), oy: -(cr.top + col.offsetTop + slot.offsetTop) };
    });
    place();
  }
  $effect(() => { void canvas.cols; void mounted; void vw; void vh; void pad; void tick().then(refreshPics); });
  const write = () => {
    frame = 0;
    if (strip) strip.style.transform = `translate3d(${-x}px,0,0)`;
    place();
    const [l, r] = canvas.max ? [1, 1] : fades();
    if (l !== fl) { fl = l; if (fadeL) fadeL.style.opacity = String(1 - l); }
    if (r !== fr) { fr = r; if (fadeR) fadeR.style.opacity = String(1 - r); }
    if (Math.abs(x - vx) > vw / 4) vx = x;
  };
  const apply = () => { if (!frame) frame = requestAnimationFrame(write); };
  const clamp = (v: number) => Math.min(Math.max(0, v), maxScroll(widths, vw));
  /** Moves the strip by `d` px (a user's pan: it takes hold of any running tween). */
  function pan(d: number) { cancelAnimationFrame(tween); tween = 0; x = clamp(x + d); apply(); settle(); }
  let settleTimer = 0;
  const settle = () => { clearTimeout(settleTimer); settleTimer = window.setTimeout(() => { vx = x; canvas.x = x; canvas.save(); }, 140); };
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.motion === "off";
  /** What the person last used, a pointer press or a key (captured on the document, so it is known the moment an action runs). */
  let last: "pointer" | "key" = "pointer";
  onMount(() => {
    const p = () => (last = "pointer"), k = () => (last = "key");
    document.addEventListener("pointerdown", p, true); document.addEventListener("keydown", k, true);
    return () => { document.removeEventListener("pointerdown", p, true); document.removeEventListener("keydown", k, true); };
  });
  /** Whether an action started now animates: a pointer drove it (not the keyboard) and motion is allowed. */
  const animates = (source: "pointer" | "key") => source === "pointer" && last === "pointer" && !reduced();
  const tokenMs = (name: string, fallback: number) => Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || fallback;
  const tokenEase = (name: string, fallback: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
  /** Eases the strip to `to` over the motion token (interruptible: a pan or another call cancels it); instant when not animated. */
  function glide(to: number, animate: boolean) {
    cancelAnimationFrame(tween); to = clamp(to);
    if (!animate || Math.abs(to - x) < 1) { x = to; apply(); settle(); return; }
    const from = x, t0 = performance.now(), ms = tokenMs("--dur-base", 220);
    const step = () => { const t = Math.max(0, Math.min(1, (performance.now() - t0) / ms)); x = from + (to - from) * (1 - (1 - t) ** 3); write(); if (t < 1) tween = requestAnimationFrame(step); else { tween = 0; settle(); } };
    tween = requestAnimationFrame(step);
  }
  /** Brings a window's column fully into view (centred when it is wider than the canvas). */
  const reveal = (id: string, animate: boolean) => { const i = canvas.colIndexOf(id); if (i >= 0) glide(scrollTo(widths, i, x, vw) - (pad ? 0 : 0), animate); };

  // Mounting is expensive (a thread view is a whole subtree), so a column is mounted a viewport before it can be seen, one column per animation frame, and unmounted only once it is two viewports away: panning back and forth never mounts the same column twice,
  // and a long pan never mounts two at once.
  let mounted = $state<string[]>([]);
  let draining = 0;
  $effect(() => {
    const ahead = nearColumns(widths, vx, vw, vw, pad).map((i) => canvas.cols[i]!.id), keep = new Set(nearColumns(widths, vx, vw, 2 * vw, pad).map((i) => canvas.cols[i]!.id));
    const have = untrack(() => mounted).filter((id) => keep.has(id));
    const missing = ahead.filter((id) => !have.includes(id));
    if (have.length !== untrack(() => mounted).length) mounted = have;
    if (!missing.length || draining) return;
    // The columns in view come at once (nothing to wait for), the ones ahead of them one by one.
    const inView = new Set(nearColumns(widths, vx, vw, 0, pad).map((i) => canvas.cols[i]!.id));
    const now = missing.filter((id) => inView.has(id));
    if (now.length) { mounted = [...have, ...now]; return; }
    draining = requestAnimationFrame(() => { draining = 0; mounted = [...untrack(() => mounted), missing[0]!]; });
  });
  const isNear = (id: string) => mounted.includes(id) || canvas.cols.find((c) => c.id === id)?.wins.some((w) => w.id === canvas.focus) === true; // the focused column is always mounted (a new window appears at once)
  const maxed = $derived(canvas.win(canvas.max));

  // ---- FLIP: the columns slide and resize from where they were (transform only), for pointer-initiated changes ----
  type Box = { x: number; y: number; w: number; h: number };
  const measure = (): Map<string, Box> => { const m = new Map<string, Box>(); for (const c of strip?.querySelectorAll<HTMLElement>("[data-col]") ?? []) { const r = c.getBoundingClientRect(); m.set(c.dataset.col!, { x: r.x, y: r.y, w: r.width, h: r.height }); } return m; };
  /**
   * Runs `mutate` (a change of the layout) and, when `animate`, plays each column from its old box to its new one on `--dur-morph` / `--ease-morph`, and a new column in by growing and fading from its place on `--dur-base`. The old boxes are read with
   * any running animation included, and that animation is cancelled before the change, so a second action continues from where the first had got to (interruptible).
   */
  async function transact(mutate: () => void, animate: boolean) {
    if (!animate) { mutate(); return; }
    const before = measure();
    for (const c of strip?.querySelectorAll<HTMLElement>("[data-col]") ?? []) c.getAnimations().forEach((a) => a.cancel());
    mutate();
    await tick();
    requestAnimationFrame(() => {
      const morph = { duration: tokenMs("--dur-morph", 350), easing: tokenEase("--ease-morph", "ease-out"), fill: "none" as const };
      for (const c of strip?.querySelectorAll<HTMLElement>("[data-col]") ?? []) {
        const old = before.get(c.dataset.col!), r = c.getBoundingClientRect();
        if (!old) { c.animate([{ opacity: 0, transform: "scale(0.96)" }, { opacity: 1, transform: "none" }], { duration: tokenMs("--dur-base", 220), easing: tokenEase("--ease-out", "ease-out") }); continue; }
        const dx = old.x - r.x, dy = old.y - r.y, sx = old.w / r.width, sy = old.h / r.height;
        if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5 && Math.abs(sx - 1) < 0.005 && Math.abs(sy - 1) < 0.005) continue;
        c.animate([{ transformOrigin: "0 0", transform: `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})` }, { transformOrigin: "0 0", transform: "none" }], morph);
      }
    });
  }

  // ---- addresses: the focused window is the address ----
  $effect(() => {
    const [k, item] = segments();
    if (!k || !item || !appFor(k) || k === "settings") return;
    const spaceKey = spaces.current.id; void spaceKey;
    untrack(() => { const had = !!canvas.find(k, item); const run = () => { const w = canvas.show(k, item); queueMicrotask(() => reveal(w.id, animates("pointer"))); }; if (had) run(); else void transact(run, animates("pointer")); });
  });
  $effect(() => {
    const w = canvas.win(canvas.focus);
    if (!w) return;
    const a = canvas.address(w);
    if (untrack(() => route.path) !== a) navigate(a);
  });
  // The app behind a window starts when its first window opens (Notes loads its list and space); the canvas needs a note to seed, so it starts Notes at once.
  $effect(() => { for (const k of new Set(canvas.all().map((w) => w.kind))) void appFor(k)?.start?.(); });
  onMount(() => { void appFor("notes")?.start?.(); });
  // First visit: a thread, a second thread beside it and a note below that, so there is something to arrange (the person's own arrangement is kept after that).
  let seeded = (() => { try { return localStorage.getItem("cal.canvas.seeded.2") === "1"; } catch { return true; } })();
  $effect(() => {
    if (seeded) return;
    const open = untrack(() => new Set(canvas.all().map((w) => `${w.kind}/${w.item}`)));
    const mine = store.threads.filter((t) => t.spaceId === spaces.current.id && !t.parentId && !open.has(`threads/${t.id}`));
    const n = notes.list[0];
    const need = untrack(() => (canvas.cols.length === 0 ? 2 : 1));
    if (mine.length < need || !n) return;
    seeded = true; try { localStorage.setItem("cal.canvas.seeded.2", "1"); } catch { /* storage is off */ }
    untrack(() => {
      const keep = canvas.focus;
      if (canvas.cols.length === 0) canvas.addRight("threads", mine.shift()!.id);
      const first = canvas.focus;
      canvas.addRight("threads", mine[0]!.id);
      canvas.addBelow("notes", n.id);
      canvas.setFocus(keep ?? first!);
    });
  });

  // ---- the wheel and touch ----
  const winOf = (t: EventTarget | null) => (t as Element | null)?.closest?.("[data-win]") as HTMLElement | null;
  const locked = () => canvas.all().some((w) => w.locked);
  /** Whether something between `t` and its window can scroll sideways in the direction of `dx`. */
  function canScrollSideways(t: EventTarget | null, win: HTMLElement, dx: number): boolean {
    for (let n = t as Element | null; n && n !== win.parentElement; n = n.parentElement) {
      const cs = getComputedStyle(n);
      if (!/(auto|scroll)/.test(cs.overflowX) || n.scrollWidth <= n.clientWidth + 1) continue;
      if (dx < 0 ? n.scrollLeft > 0 : n.scrollLeft + n.clientWidth < n.scrollWidth - 1) return true;
    }
    return false;
  }
  function wheel(e: WheelEvent) {
    if (e.ctrlKey || canvas.max) return;
    const w = winOf(e.target);
    const sideways = Math.abs(e.deltaX) > Math.abs(e.deltaY);
    if (w && (w.hasAttribute("data-focused") || w.hasAttribute("data-locked"))) {
      // Inside the focused window, always; except a sideways scroll that nothing in it can take, which pans the canvas (never while the window is locked).
      if (w.hasAttribute("data-locked") || !sideways || canScrollSideways(e.target, w, e.deltaX)) return;
    }
    if (locked()) { e.preventDefault(); e.stopPropagation(); return; }
    e.preventDefault(); e.stopPropagation(); // the canvas has it: never the shell's sidebar pull
    pan(e.deltaMode === 1 ? (e.deltaX || e.deltaY) * 32 : sideways ? e.deltaX : e.deltaY);
  }
  let fingers: { x: number; y: number } | null = null, vel = 0, lastT = 0, fling = 0;
  const mean = (t: TouchList) => ({ x: (t[0]!.clientX + t[1]!.clientX) / 2, y: (t[0]!.clientY + t[1]!.clientY) / 2 });
  function tstart(e: TouchEvent) {
    cancelAnimationFrame(fling);
    if (e.touches.length === 2 && !locked() && !canvas.max) { fingers = mean(e.touches); vel = 0; lastT = e.timeStamp; }
  }
  function tmove(e: TouchEvent) {
    if (!fingers || e.touches.length !== 2) return;
    e.preventDefault();
    const m = mean(e.touches);
    const d = fingers.x - m.x + (fingers.y - m.y); // two fingers in any direction move the canvas
    const dt = Math.max(1, e.timeStamp - lastT); vel = 0.8 * vel + 0.2 * (d / dt); lastT = e.timeStamp;
    fingers = m; pan(d);
  }
  function tend(e: TouchEvent) {
    if (!fingers || e.touches.length >= 2) return;
    fingers = null;
    let v = vel * 16;
    if (Math.abs(v) < 1 || reduced()) return;
    const step = () => { v *= 0.95; if (Math.abs(v) < 0.4) return; pan(v); fling = requestAnimationFrame(step); };
    fling = requestAnimationFrame(step);
  }
  onMount(() => {
    const n = el!;
    n.addEventListener("wheel", wheel, { passive: false });
    n.addEventListener("touchstart", tstart, { passive: true });
    n.addEventListener("touchmove", tmove, { passive: false });
    n.addEventListener("touchend", tend, { passive: true });
    const ro = new ResizeObserver(() => { vw = n.clientWidth; vh = n.clientHeight; x = clamp(x); apply(); });
    ro.observe(n);
    vw = n.clientWidth; vh = n.clientHeight;
    write();
    return () => { n.removeEventListener("wheel", wheel); n.removeEventListener("touchstart", tstart); n.removeEventListener("touchmove", tmove); n.removeEventListener("touchend", tend); ro.disconnect(); cancelAnimationFrame(frame); cancelAnimationFrame(tween); cancelAnimationFrame(fling); };
  });

  // ---- windows ----
  const iconOf = (kind: string): IconName => (appFor(kind)?.icon ?? "apps") as IconName;
  const titleOf = (kind: string, item: string) => (kind === "threads" ? store.get(item)?.title : notes.list.find((n) => n.id === item)?.title) ?? appFor(kind)?.name ?? kind;
  /** The next thread or note that has no window yet. */
  function nextOf(kind: string): string | null {
    const open = new Set(canvas.all().filter((w) => w.kind === kind).map((w) => w.item));
    const ids = kind === "threads" ? store.threads.filter((t) => t.spaceId === spaces.current.id && !t.parentId).map((t) => t.id) : notes.list.map((n) => n.id);
    return ids.find((i) => !open.has(i)) ?? null;
  }
  const chordLabel = (id: string) => { const c = shortcuts.chordFor(id); return c ? showChord(c) : ""; };
  const kbd = $derived({ max: chordLabel("win-max"), lock: chordLabel("win-lock") });

  let menu = $state<{ x: number; y: number; id: string } | null>(null);
  const menuWin = $derived(canvas.win(menu?.id ?? null));
  const menuItems = $derived.by<MenuItem[]>(() => {
    const w = menuWin;
    if (!w) return [];
    const c = canvas.cols[canvas.colIndexOf(w.id)];
    const next = (k: string) => nextOf(k);
    const open: MenuItem[] = ["threads", "notes"].flatMap((k) => (next(k) ? [{ id: `beside:${k}`, label: `Open ${appFor(k)?.name === "Threads" ? "a thread" : "a note"} beside`, icon: "sidebar-right" as IconName }, { id: `below:${k}`, label: `Open ${k === "threads" ? "a thread" : "a note"} below`, icon: "down" as IconName }] : []));
    const p = c ? proportionOf(c, canvas.cols.length, vw) : 1;
    const widthsList: MenuItem[] = narrow ? [] : PRESETS.map((x) => ({ id: `width:${x}`, label: `Width ${x === 1 ? "full" : x === 1 / 3 ? "1/3" : x === 1 / 2 ? "1/2" : "2/3"}`, checked: Math.abs(p - x) < 0.01 }));
    const rest: MenuItem[] = [
      ...(c && c.wins.length > 1 ? [{ id: "equal", label: "Equal heights" }] : []),
      ...(["card", "clear", "solid"] as Look[]).map((l) => ({ id: `look:${l}`, label: `Look: ${l === "card" ? "Card" : l === "clear" ? "Transparent" : "Solid"}`, checked: w.look === l })),
      { id: "lock", label: w.locked ? "Unlock input" : "Lock input", icon: "lock" },
      ...((appFor(w.kind)?.panels.length ?? 0) > 0 ? [{ id: "panel", label: "Open side panel", icon: "sidebar-right" as IconName }] : []),
    ];
    const thread: MenuItem[] = w.kind === "threads" && store.get(w.item) ? threadItems(store.get(w.item)!).map((i) => ({ ...i, id: `t:${i.id}` })) : [];
    return [...open, ...widthsList, ...rest, ...thread, { id: "close", label: "Close window", icon: "close", danger: true }];
  });

  function addBeside(kind: string, below: boolean, source: "pointer" | "key") {
    const item = nextOf(kind);
    if (!item) return;
    void transact(() => { const w = below ? canvas.addBelow(kind, item) : canvas.addRight(kind, item); queueMicrotask(() => reveal(w.id, animates(source))); }, animates(source));
  }
  function setPreset(id: string, p: number | null, source: "pointer" | "key") {
    const ci = canvas.colIndexOf(id);
    if (ci < 0) return;
    void transact(() => { canvas.setProportion(canvas.cols[ci]!.id, p); void tick().then(() => reveal(id, false)); }, animates(source));
  }
  function closeWin(id: string, source: "pointer" | "key") {
    if (!animates(source)) { canvas.close(id); return; }
    // The window fades and shrinks out on `--dur-base`, then the rest close up (a second FLIP).
    const node = el?.querySelector<HTMLElement>(`[data-win="${id}"]`);
    const out = node?.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: "scale(0.96)" }], { duration: tokenMs("--dur-base", 220), easing: tokenEase("--ease-out", "ease-out"), fill: "forwards" });
    const done = () => void transact(() => { canvas.close(id); queueMicrotask(() => { const f = canvas.win(canvas.focus); if (f) reveal(f.id, true); }); }, true);
    if (out) { out.onfinish = done; out.oncancel = done; } else done();
  }
  function toggleMax(id: string, source: "pointer" | "key") {
    void transact(() => canvas.toggleMax(id), animates(source));
  }
  function share(w: { kind: string; item: string }) {
    const url = location.origin + href(`/${w.kind}/${w.item}`);
    void navigator.clipboard?.writeText(url).then(() => toast("Link copied"), () => toast("Could not copy the link"));
  }
  function act(id: string, a: WinAction, source: "pointer" | "key", target: HTMLElement) {
    const w = canvas.win(id);
    if (!w) return;
    if (a === "max") toggleMax(id, source);
    else if (a === "share") share(w);
    else if (a === "lock") canvas.toggleLock(id);
    else { const r = target.getBoundingClientRect(); menu = { x: r.right, y: r.bottom, id }; }
  }
  function pick(item: string) {
    const m = menu; menu = null;
    if (!m) return;
    const w = canvas.win(m.id);
    if (!w) return;
    const source = "pointer" as const;
    if (item.startsWith("beside:")) addBeside(item.slice(7), false, source);
    else if (item.startsWith("below:")) addBeside(item.slice(6), true, source);
    else if (item.startsWith("width:")) setPreset(w.id, Number(item.slice(6)), source);
    else if (item === "equal") canvas.equalHeights(canvas.cols[canvas.colIndexOf(w.id)]!.id);
    else if (item.startsWith("look:")) canvas.setLook(w.id, item.slice(5) as Look);
    else if (item === "lock") canvas.toggleLock(w.id);
    else if (item === "panel") { if (shell.narrow) shell.over("panel"); else shell.toggleRight(); }
    else if (item === "close") closeWin(w.id, source);
    else if (item.startsWith("t:")) { const t = store.get(w.item); if (t) runThreadItem(item.slice(2), t, () => toast("Rename it from the sidebar: double-click its row")); }
  }
  /** Focus follows the press; a column not fully in view slides into view. */
  const focusWin = (id: string) => { if (canvas.focus !== id) { canvas.setFocus(id); if (!locked()) reveal(id, true); } };
  const resizeWidth = (colId: string, i: number, dx: number) => {
    const c = canvas.cols.find((k) => k.id === colId);
    if (!c) return;
    canvas.setProportion(colId, proportionFor(widths[i]! + dx, vw));
  };

  // ---- the keyboard: instant ----
  onMount(() => {
    const mine = [
      { id: "win-left", label: "Focus the column on the left", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+ArrowLeft", run: () => { const w = canvas.focusColumn(-1); if (!w) return false; reveal(w.id, false); } },
      { id: "win-right", label: "Focus the column on the right", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+ArrowRight", run: () => { const w = canvas.focusColumn(1); if (!w) return false; reveal(w.id, false); } },
      { id: "win-up", label: "Focus the window above", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+ArrowUp", run: () => (canvas.focusRow(-1) ? undefined : false) },
      { id: "win-down", label: "Focus the window below", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+ArrowDown", run: () => (canvas.focusRow(1) ? undefined : false) },
      { id: "win-move-left", label: "Move the column left", group: "Canvas" as const, where: "Anywhere", chord: "Mod+Alt+Shift+ArrowLeft", run: () => { canvas.moveColumn(-1); const f = canvas.focus; if (f) void tick().then(() => reveal(f, false)); } },
      { id: "win-move-right", label: "Move the column right", group: "Canvas" as const, where: "Anywhere", chord: "Mod+Alt+Shift+ArrowRight", run: () => { canvas.moveColumn(1); const f = canvas.focus; if (f) void tick().then(() => reveal(f, false)); } },
      { id: "win-preset", label: "Cycle the column's width presets", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+KeyR", run: () => { const f = canvas.focus, c = canvas.cols[canvas.colIndexOf(f)]; if (!f || !c) return false; setPreset(f, nextPreset(proportionOf(c, canvas.cols.length, vw)), "key"); } },
      { id: "win-max", label: "Full screen the window", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+KeyF", run: () => { const f = canvas.focus; if (!f) return false; toggleMax(f, "key"); } },
      { id: "win-lock", label: "Lock or unlock input in the window", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+KeyL", run: () => { const f = canvas.focus; if (!f) return false; canvas.toggleLock(f); } },
      { id: "win-close", label: "Close the window", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+KeyW", run: () => { const f = canvas.focus; if (!f) return false; closeWin(f, "key"); } },
      { id: "win-new", label: "Open another window beside", group: "Canvas" as const, where: "Anywhere", chord: "Alt+Shift+KeyN", run: () => { const k = nextOf("threads") ? "threads" : nextOf("notes") ? "notes" : null; if (!k) return false; addBeside(k, false, "key"); } },
    ];
    const ids = new Set(mine.map((c) => c.id));
    shortcuts.register([...shortcuts.commands.filter((c) => !ids.has(c.id)), ...mine]);
    return () => shortcuts.register(shortcuts.commands.filter((c) => !ids.has(c.id)));
  });
  void ariaKeys; void shell; void leftOf; void stripWidth; void PRESETS;
</script>

<div class="canvas" bind:this={el} data-max={maxed ? "" : undefined} data-canvas>
  <div class="fade l" bind:this={fadeL} aria-hidden="true"></div><div class="fade r" bind:this={fadeR} aria-hidden="true"></div>
  <div class="view">
    <div class="strip" bind:this={strip} style:width="{stripWidth(widths, vw) + pad}px">
      {#each canvas.cols as c, i (c.id)}
        {@const mine = c.wins.some((w) => w.id === canvas.max)}
        {#if isNear(c.id) || mine}
          <div class="col" class:max={mine} class:hidden={!!maxed && !mine} style:left="{mine ? x + edgeOf(vw) : leftOf(widths, i, vw) + pad}px" style:width={mine ? `${vw - 2 * edgeOf(vw)}px` : `${widths[i]}px`} data-col={c.id} data-l={mine ? x + edgeOf(vw) : leftOf(widths, i, vw) + pad}>
            {#each c.wins as w, k (w.id)}
              {#if !maxed || w.id === canvas.max}
                <div class="slot" style:flex="{mine ? 1 : w.weight} 1 0" style:min-height="0">
                  <WindowFrame win={w} title={titleOf(w.kind, w.item)} icon={iconOf(w.kind)} ownTitle={w.kind === "threads"} focused={canvas.focus === w.id} maximised={mine} look={w.look} {colours}
                    paper={page.paper.join(" ")} {density} hasBelow={k < c.wins.length - 1} edge={i === 0 && i === canvas.cols.length - 1 ? "lr" : i === 0 ? "l" : i === canvas.cols.length - 1 ? "r" : ""} {kbd}
                    onfocus={() => focusWin(w.id)} onaction={(a, source, target) => act(w.id, a, source, target)}
                    onresizew={(dx) => resizeWidth(c.id, i, dx)} onresizeh={(dy) => canvas.dragBoundary(c.id, k, dy, el?.querySelector<HTMLElement>(`[data-col="${c.id}"]`)?.clientHeight ?? vh)}>
                    {#if w.kind === "notes" && canvas.focus !== w.id}<NotePreview item={w.item} />
                    {:else}{@const A = appFor(w.kind)}{#if A}<A.Main id={w.item} />{/if}{/if}
                  </WindowFrame>
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      {/each}
    </div>
  </div>
  {#if canvas.cols.length === 0}<p class="none">Open a thread or a note from the sidebar.</p>{/if}
</div>

<Menu open={!!menu} items={menuItems} label="Window menu" anchor={menu ? { left: menu.x, top: menu.y, right: menu.x, bottom: menu.y } : null} placement="bottom" onclose={() => (menu = null)} onpick={pick} />

<style>
  /* The area between the sidebars is the canvas and paints nothing: behind the windows is the one veiled wallpaper layer the sidebars sit on (shell/Backdrop.svelte), so every pixel outside the windows is the same with and without them. The windows are middle cards. */
  .canvas { position: absolute; inset: 0; overflow: hidden; touch-action: pan-x pan-y; overscroll-behavior: contain; }
  /* The edge fade (the owner's: minimal, only while a window enters or leaves through an edge, never a standing band): two 28 px strips at the area's left and right edges that paint the same layers as the background (the picture, the veil), fixed to the viewport so they match it,
     under a static gradient, drawn over the windows. Their opacity is 0 unless a window crosses that edge (`fades()`), so nothing is drawn at rest; it is the only thing that changes while panning, and opacity is composited. */
  .fade { position: absolute; z-index: 2; top: 0; bottom: 0; width: 28px; pointer-events: none; opacity: 0; background: var(--veil-l, linear-gradient(rgb(var(--tint) / 0.5) 0 0)), var(--wall-side, var(--wall-fallback, var(--backdrop))) center / cover no-repeat fixed; }
  .fade.l { left: 0; -webkit-mask-image: linear-gradient(to right, #000, transparent); mask-image: linear-gradient(to right, #000, transparent); }
  .fade.r { right: 0; -webkit-mask-image: linear-gradient(to left, #000, transparent); mask-image: linear-gradient(to left, #000, transparent); }
  .canvas[data-max] .fade { display: none; }
  .view { position: absolute; inset: 0; z-index: 1; }
  .strip { position: absolute; top: 0; bottom: 0; left: 0; will-change: transform; }
  /* Columns run from the top to the bottom edge of the area as today's card does; the gap between stacked windows is the same 8 px. */
  .col { position: absolute; top: 0; bottom: 0; display: flex; flex-direction: column; gap: 8px; }
  .col.max { top: 0; bottom: 0; }
  .col.hidden { display: none; }
  .slot { position: relative; min-height: 0; }
  .none { position: absolute; inset: 0; z-index: 1; display: grid; place-items: center; color: var(--ink-2); font-size: var(--text-ui); }
</style>
