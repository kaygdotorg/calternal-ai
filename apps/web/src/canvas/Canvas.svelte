<script lang="ts">
  // The canvas experiment (docs/ui-shell-status.md): inside the middle card, an endless horizontal strip of columns (niri's model) holding windows, each an app's item. Behind `?canvas=1` in the fixtures build only.
  //
  // Panning is compositor-only: the strip is one element moved by `transform: translate3d`, written straight to its style in an animation frame, no reactive state and no layout per frame. A column is mounted only while it is
  // within half a viewport of the screen (layout.ts `nearColumns`, re-evaluated when the scroll has moved by a quarter viewport), so windows off screen are not in the DOM at all: not laid out, not painted, not styled.
  // The wheel rule: a wheel over the focused window (or a locked one) always scrolls inside it; over an unfocused window or the gaps it pans the canvas (a vertical wheel pans sideways too, since a mouse has no horizontal wheel).
  // While a window holds the input lock the canvas never moves. On touch, two fingers pan the canvas in any direction and one finger scrolls the window under it.
  import { onMount, untrack } from "svelte";
  import { currentScheme, Menu, onScheme, Pill, type MenuItem } from "@calternal/ui";
  import { appFor } from "../apps/registry";
  import { store } from "../apps/threads/store.svelte";
  import { notes } from "../apps/notes/store.svelte";
  import { navigate, route, segments } from "../lib/route.svelte";
  import { onWallpaperColours } from "../appearance/wallpaper";
  import { spaces } from "../shell/spaces.svelte";
  import { GAP, colLeft, maxScroll, nearColumns, scrollTo, stripWidth } from "./layout";
  import { canvas } from "./model.svelte";
  import { hueOf, windowColours } from "./surface";
  import NotePreview from "./NotePreview.svelte";
  import WindowFrame from "./WindowFrame.svelte";
  import type { IconName } from "@calternal/ui";

  let el = $state<HTMLElement>(), strip = $state<HTMLElement>();
  let vw = $state(1000), vh = $state(700);
  /** The canvas's width now (the observer's value arrives a frame late, and a window opened at start-up needs it at once). */
  const width = () => el?.clientWidth || document.querySelector<HTMLElement>(".mid")?.clientWidth || vw;
  /** The scroll position now (px); `vx` is where the mounted set was last worked out, a reactive copy updated only when the scroll has moved by a quarter viewport. */
  let x = $state(canvas.x), vx = $state(canvas.x);
  let fadeL = $state<HTMLElement>(), fadeR = $state<HTMLElement>();
  let frame = 0, tween = 0;
  const BAR = 36;

  // ---- colours: the wallpaper's hue and the scheme ----
  let scheme = $state(currentScheme());
  let hue = $state<number | null>(hueOf(getComputedStyle(document.documentElement).getPropertyValue("--accent-base").trim() || "#808080"));
  onMount(() => {
    const a = onScheme((s) => (scheme = s));
    const b = onWallpaperColours((c) => { hue = c?.hue?.h ?? hueOf(getComputedStyle(document.documentElement).getPropertyValue("--accent-base").trim() || "#808080"); });
    return () => { a(); b(); };
  });
  const colours = $derived(windowColours(scheme, hue));

  // ---- the strip ----
  /** The edge fade: how far a window is cut off at the left and right edge of the viewport, as the complement of the edge strip's opacity (1: no fade). It is 1 at both edges unless a window crosses that edge; it falls to 0 over `FADE` px of crossing. Arithmetic over the mounted columns only. */
  const FADE = 24;
  function fades(): [number, number] {
    let l = 1, r = 1, left = GAP;
    for (const c of canvas.cols) {
      const right = left + c.w;
      if (left < x && right > x) l = Math.min(l, 1 - Math.min(1, (x - left) / FADE));
      if (right > x + vw && left < x + vw) r = Math.min(r, 1 - Math.min(1, (right - (x + vw)) / FADE));
      left = right + GAP;
      if (left > x + 2 * vw) break;
    }
    return [l, r];
  }
  let fl = 1, fr = 1;
  const write = () => {
    frame = 0;
    if (strip) strip.style.transform = `translate3d(${-x}px,0,0)`;
    const [l, r] = fades();
    if (l !== fl) { fl = l; if (fadeL) fadeL.style.opacity = String(1 - l); }
    if (r !== fr) { fr = r; if (fadeR) fadeR.style.opacity = String(1 - r); }
    if (Math.abs(x - vx) > vw / 4) vx = x;
  };
  const apply = () => { if (!frame) frame = requestAnimationFrame(write); };
  const clamp = (v: number) => Math.min(Math.max(0, v), maxScroll(canvas.cols, vw));
  /** Moves the strip by `d` px (a user's pan: it takes hold of any running tween). */
  function pan(d: number) { cancelAnimationFrame(tween); tween = 0; x = clamp(x + d); apply(); settle(); }
  let settleTimer = 0;
  const settle = () => { clearTimeout(settleTimer); settleTimer = window.setTimeout(() => { vx = x; canvas.x = x; canvas.save(); }, 140); };
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.motion === "off";
  /** Eases the strip to `to` over the motion token (interruptible: a pan or another call cancels it). */
  function glide(to: number) {
    cancelAnimationFrame(tween); to = clamp(to);
    if (reduced() || Math.abs(to - x) < 1) { x = to; apply(); settle(); return; }
    const from = x, t0 = performance.now(), ms = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--dur-base")) || 220;
    const step = (now: number) => { const t = Math.min(1, (now - t0) / ms); x = from + (to - from) * (1 - (1 - t) ** 3); write(); if (t < 1) tween = requestAnimationFrame(step); else { tween = 0; settle(); } };
    tween = requestAnimationFrame(step);
  }
  /** Brings a window's column fully into view. */
  const reveal = (id: string) => { const i = canvas.colIndexOf(id); if (i >= 0) glide(scrollTo(canvas.cols, i, x, vw)); };

  // Mounting is expensive (a thread view is a whole subtree), so a column is mounted a viewport before it can be seen and unmounted only once it is two viewports away: panning back and forth never mounts the same column twice.
  let mounted = $state<string[]>([]);
  $effect(() => {
    const ahead = nearColumns(canvas.cols, vx, vw, vw).map((i) => canvas.cols[i]!.id), keep = new Set(nearColumns(canvas.cols, vx, vw, 2 * vw).map((i) => canvas.cols[i]!.id));
    const next = [...new Set([...untrack(() => mounted).filter((id) => keep.has(id)), ...ahead])];
    if (next.length !== untrack(() => mounted).length || next.some((id, k) => id !== untrack(() => mounted)[k])) mounted = next;
  });
  const isNear = (id: string) => mounted.includes(id);
  const maxed = $derived(canvas.win(canvas.max));

  // ---- addresses: the focused window is the address ----
  $effect(() => {
    const [k, item] = segments();
    if (!k || !item || !appFor(k) || k === "settings") return;
    const spaceKey = spaces.current.id; void spaceKey;
    untrack(() => { const w = canvas.show(k, item, width()); queueMicrotask(() => reveal(w.id)); });
  });
  $effect(() => {
    const w = canvas.win(canvas.focus);
    if (!w) return;
    const a = canvas.address(w);
    if (untrack(() => route.path) !== a) navigate(a);
  });
  // The app behind a window starts when its first window opens (Notes loads its list and space).
  $effect(() => { for (const k of new Set(canvas.all().map((w) => w.kind))) void appFor(k)?.start?.(); });
  // First visit: a second thread beside the first and a note below it, so there is something to arrange (the person's own arrangement is kept after that).
  onMount(() => { void appFor("notes")?.start?.(); });
  let seeded = (() => { try { return localStorage.getItem("cal.canvas.seeded") === "1"; } catch { return true; } })();
  $effect(() => {
    if (seeded || canvas.cols.length === 0) return;
    const open = untrack(() => new Set(canvas.all().map((w) => `${w.kind}/${w.item}`)));
    const t = store.threads.find((x) => x.spaceId === spaces.current.id && !x.parentId && !open.has(`threads/${x.id}`));
    const n = notes.list[0];
    if (!t || !n) return;
    seeded = true; try { localStorage.setItem("cal.canvas.seeded", "1"); } catch { /* storage is off */ }
    untrack(() => { const keep = canvas.focus; canvas.addRight("threads", t.id, width()); canvas.addBelow("notes", n.id, width()); if (keep) canvas.setFocus(keep); });
  });

  // ---- the wheel and touch ----
  const winOf = (t: EventTarget | null) => (t as Element | null)?.closest?.("[data-win]") as HTMLElement | null;
  const locked = () => canvas.all().some((w) => w.locked);
  function wheel(e: WheelEvent) {
    if (e.ctrlKey || canvas.max) return;
    const w = winOf(e.target);
    if (w && (w.hasAttribute("data-focused") || w.hasAttribute("data-locked"))) return; // inside the window, always
    if (locked()) { e.preventDefault(); return; }                                      // a lock holds all input in its window: the canvas does not move
    e.preventDefault();
    pan(e.deltaMode === 1 ? (e.deltaX || e.deltaY) * 32 : (Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY));
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
    const d = fingers.x - m.x + (fingers.y - m.y); // two fingers in any direction move the canvas (right or down moves on, left or up goes back)
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
    const ro = new ResizeObserver(() => { vw = n.clientWidth; vh = n.clientHeight - BAR - GAP; x = clamp(x); apply(); });
    ro.observe(n);
    vw = n.clientWidth; vh = n.clientHeight - BAR - GAP;
    write();
    return () => { n.removeEventListener("wheel", wheel); n.removeEventListener("touchstart", tstart); n.removeEventListener("touchmove", tmove); n.removeEventListener("touchend", tend); ro.disconnect(); cancelAnimationFrame(frame); cancelAnimationFrame(tween); cancelAnimationFrame(fling); };
  });

  // ---- actions ----
  const iconOf = (kind: string): IconName => (appFor(kind)?.icon ?? "apps") as IconName;
  const titleOf = (kind: string, item: string) => (kind === "threads" ? store.get(item)?.title : notes.list.find((n) => n.id === item)?.title) ?? appFor(kind)?.name ?? kind;
  /** The next thread or note that has no window yet. */
  function nextOf(kind: string): string | null {
    const open = new Set(canvas.all().filter((w) => w.kind === kind).map((w) => w.item));
    const ids = kind === "threads" ? store.threads.filter((t) => t.spaceId === spaces.current.id && !t.parentId).map((t) => t.id) : notes.list.map((n) => n.id);
    return ids.find((i) => !open.has(i)) ?? null;
  }
  let menu = $state<{ x: number; y: number; where: "right" | "below" } | null>(null);
  const menuItems = $derived<MenuItem[]>(["threads", "notes"].flatMap((k) => { const n = nextOf(k); return n ? [{ id: k, label: `${appFor(k)?.name}: ${titleOf(k, n)}` }] : []; }));
  function open(kind: string, where: "right" | "below") {
    const item = nextOf(kind);
    if (!item) return;
    const w = where === "right" ? canvas.addRight(kind, item, width()) : canvas.addBelow(kind, item, width());
    queueMicrotask(() => reveal(w.id));
  }
  function act(id: string, a: "right" | "below" | "frost" | "max" | "close" | "lock", target: HTMLElement | null) {
    if (a === "right" || a === "below") { const r = target?.getBoundingClientRect(); menu = { x: r?.left ?? 0, y: r?.bottom ?? 0, where: a }; canvas.setFocus(id); }
    else if (a === "frost") canvas.toggleFrost(id);
    else if (a === "max") canvas.toggleMax(id);
    else if (a === "close") canvas.close(id);
    else canvas.toggleLock(id);
  }
  const pick = (id: string) => { const m = menu; menu = null; if (m) open(id, m.where); };
  /** Focus follows the press; a column not fully in view slides into view. */
  const focusWin = (id: string) => { if (canvas.focus !== id) { canvas.setFocus(id); if (!locked()) reveal(id); } };
</script>

<div class="canvas" bind:this={el} data-max={maxed ? "" : undefined} data-canvas>
  <div class="cbar">
    <span class="count">{canvas.all().length} {canvas.all().length === 1 ? "window" : "windows"}</span>
    <span class="grp">
      <Pill icon="plus" label="Open another window" onclick={(e) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); menu = { x: r.left, y: r.bottom, where: "right" }; }} />
      <Pill icon="spark" label={canvas.frostAll ? "Windows are frosted: make them opaque" : "Windows are opaque: make them frosted"} aria-pressed={canvas.frostAll} onclick={() => { canvas.frostAll = !canvas.frostAll; canvas.save(); }} />
    </span>
  </div>
  <div class="fade l" bind:this={fadeL} aria-hidden="true"></div><div class="fade r" bind:this={fadeR} aria-hidden="true"></div>
  <div class="view">
  <div class="strip" bind:this={strip} style:width="{stripWidth(canvas.cols)}px">
    {#each canvas.cols as c, i (c.id)}
      {@const mine = c.wins.some((w) => w.id === canvas.max)}
      {#if isNear(c.id) || mine}
        <div class="col" class:max={mine} class:hidden={!!maxed && !mine} style:left="{mine ? x : colLeft(canvas.cols, i)}px" style:width={mine ? `${vw}px` : `${c.w}px`} data-col={c.id}>
          {#each c.wins as w, k (w.id)}
            {#if !maxed || w.id === canvas.max}
              <div class="slot" style:flex="{mine ? 1 : w.weight} 1 0" style:min-height="0">
                <WindowFrame win={w} title={titleOf(w.kind, w.item)} icon={iconOf(w.kind)} focused={canvas.focus === w.id} maximised={mine} frosted={w.frosted || canvas.frostAll} {colours} hasBelow={k < c.wins.length - 1}
                  onfocus={() => focusWin(w.id)} onaction={(a) => act(w.id, a, document.querySelector<HTMLElement>(`[data-win="${w.id}"] .acts`))}
                  onresizew={(dx) => canvas.setWidth(c.id, c.w + dx)} onresizeh={(dy) => canvas.dragBoundary(c.id, k, dy, vh)}>
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
  {#if canvas.cols.length === 0}<p class="none">No windows. Open a thread or a note from the sidebar, or use the plus above.</p>{/if}
</div>

<Menu open={!!menu} items={menuItems} label="Open in a new window" anchor={menu ? { left: menu.x, top: menu.y, right: menu.x, bottom: menu.y } : null} placement="bottom" onclose={() => (menu = null)} onpick={pick} />

<style>
  /* The area around the windows shows the wallpaper sharp (Craft: the backdrop photo fills the area around the page at full sharpness). It is fixed to the viewport like the card's own picture, so panning never moves it. */
  .canvas { position: absolute; inset: 0; overflow: hidden; touch-action: pan-x pan-y; overscroll-behavior: contain; }
  .canvas::before { content: ""; position: absolute; inset: 0; z-index: 0; background: var(--wall-sharp, var(--wall-fallback, var(--backdrop))) center / cover no-repeat fixed; transition: opacity var(--dur-base) var(--ease-out); }
  .canvas[data-max]::before { opacity: 0; }
  .cbar { position: absolute; z-index: 3; top: 0; left: 0; right: 0; height: 36px; padding: 0 var(--s-4) 0 var(--edge-l, var(--s-4)); display: flex; align-items: center; justify-content: space-between; gap: var(--s-3); color: #fff; font-size: var(--text-label); }
  .count { padding: 2px var(--s-3); border-radius: var(--r-pill); background: rgb(0 0 0 / 0.28); backdrop-filter: none; }
  .grp { display: inline-flex; gap: var(--s-2); }
  .canvas[data-max] .cbar { display: none; }
  /* The edge fade (the owner's: minimal, only while a window enters or leaves through an edge, never a standing band): two 28 px strips at the left and right edges that paint the same sharp picture as the area around the windows (fixed to the viewport, so they match it
     exactly) under a static gradient, drawn over the windows. Their opacity is 0 unless a window crosses that edge (`fades()` above), so nothing is visible at rest; it is the only thing that changes while panning, and opacity is composited. (A mask was
     tried first: its edge stops changing per frame made the whole parent repaint, p95 50 ms at 4x CPU.) Top and bottom would be the same two strips turned; the canvas has no vertical scroll, so a window never crosses those edges and they are not drawn. */
  .fade { position: absolute; z-index: 2; top: 0; bottom: 0; width: 28px; pointer-events: none; opacity: 0; background: var(--wall-sharp, var(--wall-fallback, var(--backdrop))) center / cover no-repeat fixed; }
  .fade.l { left: 0; -webkit-mask-image: linear-gradient(to right, #000, transparent); mask-image: linear-gradient(to right, #000, transparent); }
  .fade.r { right: 0; -webkit-mask-image: linear-gradient(to left, #000, transparent); mask-image: linear-gradient(to left, #000, transparent); }
  .canvas[data-max] .fade { display: none; }
  .view { position: absolute; inset: 0; z-index: 1; }
  .strip { position: absolute; z-index: 1; top: 0; bottom: 0; left: 0; will-change: transform; }
  .col { position: absolute; top: 36px; bottom: 12px; display: flex; flex-direction: column; gap: 12px; }
  .col.max { top: 0; bottom: 0; }
  .col.hidden { display: none; }
  .slot { position: relative; min-height: 0; }
  .none { position: absolute; inset: 0; z-index: 1; display: grid; place-items: center; color: #fff; font-size: var(--text-ui); }
</style>
