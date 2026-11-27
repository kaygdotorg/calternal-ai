<script lang="ts">
  // The middle area is a canvas of windows (docs/ui-shell-status.md): a strip of columns,
  // each window the shared card. One window fills the area; two share it, half each; more
  // keep half and the strip scrolls. Shared verbatim from apps/mockup/src/canvas/Canvas.svelte.
  import type { Snippet } from "svelte";
  import { flip } from "svelte/animate";
  import { Grip, IconButton, tip, Menu, Panel, Sheet, type MenuItem } from "@calternal/ui";
  import type { Win, Windows } from "../windows.svelte";
  import { layout } from "../layout.svelte";
  import { tween, ms } from "../motion";
  import { tick, untrack } from "svelte";

  let {
    wins,
    titleOf,
    isTitleHidden = () => false,
    renderBody,
    extraMenu = () => [],
    onMenuPick,
    narrow = false,
    lights = false,
    sidebarHidden = false,
    onsidebar,
    onback,
    loneWidth = "800px",
    showRightToggle = false,
    rightSheet,
  }: {
    wins: Windows;
    titleOf: (w: Win) => string;
    isTitleHidden?: (w: Win) => boolean;
    renderBody: Snippet<[Win]>;
    extraMenu?: (w: Win) => MenuItem[];
    onMenuPick?: (w: Win, id: string) => void;
    narrow?: boolean;
    lights?: boolean;
    sidebarHidden?: boolean;
    onsidebar?: () => void;
    onback: () => void;
    loneWidth?: string;
    showRightToggle?: boolean;
    rightSheet?: Snippet<[Win | undefined, () => void]>;
  } = $props();

  let menuFor = $state<string | null>(null);
  let menuEl = $state<HTMLElement | null>(null);
  let info = $state(false);
  let area = $state<HTMLElement>();

  const activeWinForMenu = $derived(menuFor ? wins.list.find((x) => x.id === menuFor) ?? null : null);
  const menu = $derived<MenuItem[]>([
    ...(activeWinForMenu ? extraMenu(activeWinForMenu) : []),
    { id: "__full", label: wins.full ? "Exit full screen" : "Full screen", icon: "expand" },
    ...(showRightToggle ? [{ id: "__panel", label: layout.rightOpen ? "Hide the side panel" : "Show the side panel", icon: "sidebar-right" as const }] : []),
    { id: "__dup", label: "Open in new window", icon: "external" },
    { id: "__close", label: "Close", icon: "close", danger: true },
  ]);

  const pick = (id: string) => {
    const wId = menuFor;
    menuFor = null;
    if (id === "__panel") { layout.rightOpen = !layout.rightOpen; return; }
    if (!wId) return;
    const o = wins.list.find((x) => x.id === wId);
    if (!o) return;
    if (id === "__close") wins.close(wId);
    else if (id === "__dup") wins.beside(o.kind, o.ref, true);
    else if (id === "__full") wins.full = wins.full ? null : wId;
    else onMenuPick?.(o, id);
  };

  const n = $derived(wins.list.length);
  const idx = $derived(wins.list.findIndex((w) => w.id === wins.focus));
  const grow = (_: Element) => ({ duration: ms("--dur-morph", 350), css: (t: number) => `opacity:${t};scale:${0.96 + 0.04 * t};transform-origin:left center` });
  const leave = (_: Element) => ({ duration: ms("--dur-leave", 200), css: (t: number) => `opacity:${t};scale:${0.97 + 0.03 * t}` });

  function key(e: KeyboardEvent) {
    if (!e.altKey || !e.shiftKey) return;
    if (e.ctrlKey || e.metaKey) {
      const w = wins.focus, i = wins.list.findIndex((x) => x.id === w);
      if (w && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault();
        wins.moveTo(w, i + (e.key === "ArrowLeft" ? -1 : 1));
      }
      return;
    }
    if (e.key === "ArrowLeft") { e.preventDefault(); wins.step(-1); }
    else if (e.key === "ArrowRight") { e.preventDefault(); wins.step(1); }
    else if (e.code === "KeyW" && wins.focus) { e.preventDefault(); wins.close(wins.focus); }
    else if (e.code === "KeyF" && wins.focus) { e.preventDefault(); wins.full = wins.full ? null : wins.focus; }
    else if (e.code === "KeyP" && showRightToggle) { e.preventDefault(); layout.rightOpen = !layout.rightOpen; }
  }

  const back = (w: Win) => { if (n > 1) wins.close(w.id); else onback(); };

  function track(el: HTMLElement) {
    const bar = () => (el.closest(".cv-wn")?.querySelector<HTMLElement>(".wh")?.getBoundingClientRect().height ?? 0);
    const place = () => {
      for (const x of el.querySelectorAll(".ub")) x.classList.remove("ub");
      el.classList.remove("ub");
      const top = el.getBoundingClientRect().top, b = bar();
      if (!b) return;
      for (const s of el.querySelectorAll<HTMLElement>("*")) {
        const cs = getComputedStyle(s);
        if (!/auto|scroll/.test(cs.overflowY) || s.clientHeight < 80) continue;
        if (Math.abs(s.getBoundingClientRect().top - top - b) <= 2) {
          s.classList.add("ub"); el.classList.add("ub"); break;
        }
      }
    };
    const mo = new MutationObserver(() => place());
    mo.observe(el, { childList: true, subtree: true });
    queueMicrotask(() => place());
    addEventListener("resize", place);
    return { destroy() { mo.disconnect(); removeEventListener("resize", place); } };
  }

  const MIN_W = 360, QUIET = 110, PRESETS = [1 / 3, 1 / 2, 2 / 3], SNAP = 12;
  const gap = () => (area ? parseFloat(getComputedStyle(area).columnGap) || 12 : 12);
  const padX = () => { if (!area) return 0; const c = getComputedStyle(area); return (parseFloat(c.paddingLeft) || 0) + (parseFloat(c.paddingRight) || 0); };
  let width = $state(1200), anchor = $state(0);
  const per = $derived(n >= 2 && width >= 2 * MIN_W + 12 ? 2 : 1);
  const frac = (i: number) => (n <= 1 || per === 1 ? 1 : i % 2 === 0 ? wins.split : 1 - wins.split);
  const last = $derived(Math.max(0, n - per));
  const beyond = $derived(wins.full || narrow ? 0 : Math.max(0, n - anchor - per));
  const cols = () => [...(area?.querySelectorAll<HTMLElement>("[data-win]") ?? [])];
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  function pos(i: number): number {
    const c = cols()[i];
    if (!area || !c) return 0;
    return Math.max(0, Math.min(area.scrollWidth - area.clientWidth, c.offsetLeft - (parseFloat(getComputedStyle(area).paddingLeft) || 0)));
  }
  let anim = 0, quiet = 0, follow = 0, down = false;
  const halt = () => { cancelAnimationFrame(anim); anim = 0; };
  const stopFollow = () => { cancelAnimationFrame(follow); follow = 0; };
  function glide(to: number) {
    halt(); if (!area) return;
    const from = area.scrollLeft, d = to - from;
    if (Math.abs(d) < 1 || reduced()) { area.scrollLeft = to; return; }
    const T = ms("--dur-morph", 350), t0 = performance.now();
    const step = (now: number) => {
      if (!area) return;
      const t = Math.min(1, (now - t0) / T);
      area.scrollLeft = from + d * (1 - (1 - t) ** 3);
      anim = t < 1 ? requestAnimationFrame(step) : 0;
    };
    anim = requestAnimationFrame(step);
  }
  function settle() {
    if (!area || narrow || wins.full || n < 2) return;
    if (down) { quiet = window.setTimeout(settle, QUIET); return; }
    const x = area.scrollLeft, from = pos(Math.min(anchor, last)), w = cols()[anchor]?.offsetWidth ?? area.clientWidth, d = x - from, slack = w * 0.15;
    let to = Math.min(anchor, last);
    if (d > slack) { to = last; for (let k = anchor + 1; k <= last; k++) if (pos(k) >= x - slack) { to = k; break; } }
    else if (d < -slack) { to = 0; for (let k = anchor - 1; k >= 0; k--) if (pos(k) <= x + slack) { to = k; break; } }
    anchor = to; glide(pos(to));
  }
  const showing = (i: number, a: number) => Math.max(0, Math.min(Math.max(0, n - per), i < a ? i : i >= a + per ? i - per + 1 : a));
  function keep() {
    if (!area || narrow) return;
    halt(); stopFollow();
    const t0 = performance.now(), T = reduced() ? 0 : ms("--dur-morph", 350) + 80;
    const step = (now: number) => {
      if (!area) return;
      area.scrollLeft = wins.full ? 0 : pos(Math.min(anchor, last));
      follow = now - t0 < T ? requestAnimationFrame(step) : 0;
    };
    follow = requestAnimationFrame(step);
  }
  let seen = { count: 0, full: null as string | null, per: 1 };
  $effect(() => {
    const id = wins.focus, count = n, f = wins.full, p = per;
    void p;
    if (narrow) return;
    untrack(() => {
      const i = wins.list.findIndex((w) => w.id === id);
      const next = showing(Math.max(0, i), anchor);
      const moved = next !== anchor, changed = count !== seen.count || f !== seen.full || p !== seen.per;
      seen = { count, full: f, per: p };
      anchor = next;
      tick().then(() => { if (changed) keep(); else if (moved) glide(pos(anchor)); });
    });
  });
  function sizes(el: HTMLElement) {
    const ro = new ResizeObserver(() => {
      const w = el.clientWidth - padX();
      if (w !== width) width = w;
      anchor = showing(Math.max(0, idx), anchor);
      keep();
    });
    ro.observe(el);
    return { destroy: () => ro.disconnect() };
  }
  let fadeL = $state(false), fadeR = $state(false), moving2 = $state(false), mt = 0;
  function scrolled() {
    if (!area || narrow || dragging) return;
    fadeL = area.scrollLeft > 2;
    fadeR = area.scrollLeft + area.clientWidth < area.scrollWidth - 2;
    moving2 = true; clearTimeout(mt); mt = window.setTimeout(() => (moving2 = false), 200);
    if (anim || follow) return;
    clearTimeout(quiet); quiet = window.setTimeout(settle, QUIET);
  }
  const grab = () => { halt(); stopFollow(); clearTimeout(quiet); };
  const press = () => { down = true; grab(); }, release = () => { if (!down) return; down = false; clearTimeout(quiet); quiet = window.setTimeout(settle, QUIET); };
  const pointerIn = (e: PointerEvent) => { if (e.pointerType !== "touch") press(); }, pointerOut = (e: PointerEvent) => { if (e.pointerType !== "touch") release(); };
  const lift = (e: TouchEvent) => { if (!e.touches.length) release(); };

  let start: { L: number; A: number; i: number } | null = null, dragging = $state(false);
  function dstart(i: number) { if (!area) return; const c = cols()[i]!.getBoundingClientRect(); start = { L: c.left, A: area.clientWidth - padX(), i }; dragging = true; }
  function setSplit(i: number, p: number) {
    if (!start) return;
    const { A } = start, lo = Math.max(0.2, (MIN_W + gap()) / (A + gap())), hi = 1 - lo;
    p = Math.max(lo, Math.min(hi, p));
    for (const q of PRESETS) if (q >= lo && q <= hi && Math.abs((q - p) * (A + gap())) < SNAP) p = q;
    wins.split = i % 2 === 0 ? p : 1 - p;
    tick().then(() => { if (area) area.scrollLeft = pos(anchor); });
  }
  function dmove(i: number, e: PointerEvent) { if (!start) dstart(i); setSplit(i, (e.clientX - start!.L - gap() / 2 + gap()) / (start!.A + gap())); }
  const dend = () => { start = null; dragging = false; };
  function dkey(i: number, e: KeyboardEvent) {
    const A = area!.clientWidth - padX(), d = (e.shiftKey ? 64 : 16) / (A + gap());
    let p = frac(i);
    if (e.key === "ArrowLeft") p -= d;
    else if (e.key === "ArrowRight") p += d;
    else if (e.key === "Home") p = 0;
    else if (e.key === "End") p = 1;
    else if (e.key === "Enter") { wins.split = 0.5; keep(); e.preventDefault(); return; }
    else return;
    e.preventDefault(); dstart(i); setSplit(i, p); start = null; dragging = false;
  }

  let moving = $state<string | null>(null);
  function tdown(e: PointerEvent, w: Win) {
    if (e.button !== 0 || narrow || n < 2) return;
    const x0 = e.clientX; let on = false;
    const mv = (m: PointerEvent) => {
      if (!on) { if (Math.abs(m.clientX - x0) < 5) return; on = true; moving = w.id; document.documentElement.style.userSelect = "none"; }
      const cs = [...(area?.querySelectorAll<HTMLElement>("[data-win]") ?? [])];
      let to = cs.findIndex((c) => { const b = c.getBoundingClientRect(); return m.clientX < b.left + b.width / 2; });
      if (to < 0) to = cs.length - 1;
      if (to !== wins.list.findIndex((x) => x.id === w.id)) wins.moveTo(w.id, to);
    };
    const up = () => { removeEventListener("pointermove", mv); removeEventListener("pointerup", up); moving = null; document.documentElement.style.userSelect = ""; };
    addEventListener("pointermove", mv); addEventListener("pointerup", up);
  }
</script>

{#snippet tl(w: Win)}
  <span class="tl" role="group" aria-label="Window" data-tl>
    {#if n > 1}<button type="button" class="tb red" use:tip={{ label: "Close this window" }} aria-label="Close this window" onclick={() => wins.close(w.id)}><svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.5 1.5l5 5M6.5 1.5l-5 5" /></svg></button>{/if}
    <button type="button" class="tb green" class:max={wins.full === w.id} use:tip={{ label: wins.full === w.id ? "Exit full screen" : "Full screen" }} aria-label={wins.full === w.id ? "Exit full screen" : "Full screen"} aria-pressed={wins.full === w.id} onclick={() => (wins.full = wins.full === w.id ? null : w.id)}><svg viewBox="0 0 8 8" aria-hidden="true">{#if wins.full === w.id}<path d="M7 1v3.2H3.8zM1 7V3.8h3.2z" />{:else}<path d="M1 1h3.2L1 4.2zM7 7H3.8L7 3.8z" />{/if}</svg></button>
  </span>
{/snippet}

<svelte:window onkeydown={key} onpointerup={pointerOut} onpointercancel={pointerOut} ontouchend={lift} ontouchcancel={lift} />
<div class="cvw">
<div class="canvas" use:sizes onwheel={grab} onpointerdown={pointerIn} ontouchstart={press} style:--lone-w={loneWidth} onscroll={scrolled} class:moving={moving2} class:fl={fadeL} class:fr={fadeR} class:phone={narrow} class:full={!!wins.full} class:resizing={dragging} bind:this={area} data-canvas data-windows={n} data-per={per} data-anchor={anchor}>
  {#each wins.list as w, i (w.id)}
    <div class="col" class:focus={w.id === wins.focus} class:hide={!!wins.full && wins.full !== w.id} class:moving={moving === w.id} style:--p={wins.full === w.id ? 1 : frac(i)} style:--d={narrow ? i - idx : 0} data-win data-win-kind={w.kind} data-win-ref={w.ref} data-focused={w.id === wins.focus} data-frac={frac(i).toFixed(3)} animate:flip={tween()} in:grow out:leave onpointerdowncapture={() => (wins.focus = w.id)}>
      <Panel radius="card" class="cv-wn">
        <div class="wh" class:lights={lights && i === 0} class:desk={!narrow}>
          {#if narrow}
            <IconButton icon="back" label={n > 1 ? "Close this window" : "Back to the sidebar"} onclick={() => back(w)} />
          {:else}
            <span class="wl">
              {#if sidebarHidden && i === 0}<Panel radius="pill" class="roundP tbp cv-sb"><IconButton icon="sidebar" label="Show the sidebar" aria-keyshortcuts="Control+Meta+S" onclick={() => onsidebar?.()} /></Panel>{/if}
              <Panel radius="pill" class="cv-lp tbp">{@render tl(w)}</Panel>
            </span>
          {/if}
          <Panel radius="pill" class={`cv-tp tbp${isTitleHidden(w) ? " hid" : ""}`}>
            <span class="tt" class:grab={!narrow && n > 1} role="presentation" title={n > 1 && !narrow ? "Drag to move this window" : undefined} onpointerdown={(e) => tdown(e, w)}>
              <span class="tx" data-open-title={w.id === wins.focus ? "" : undefined}>{titleOf(w)}</span>
            </span>
          </Panel>
          {#if narrow}
            <span class="sp"></span>
            {#if rightSheet}
              <IconButton icon="info" label="Details" onclick={() => (info = true)} />
            {:else}
              <IconButton icon="more" label="Window menu" onclick={(e: MouseEvent) => { menuEl = e.currentTarget as HTMLElement; menuFor = w.id; }} />
            {/if}
          {:else}
            <span class="wr">
              <Panel radius="pill" class="cv-bp tbp">
                <span class="bt">
                  <IconButton icon="share" label="Share" onclick={() => {}} />
                  <IconButton icon="more" label="Window menu" onclick={(e: MouseEvent) => { menuEl = e.currentTarget as HTMLElement; menuFor = w.id; }} />
                </span>
              </Panel>
              {#if showRightToggle && i === n - 1}
                <Panel radius="pill" class="roundP tbp">
                  <IconButton icon="sidebar-right" label={layout.rightOpen ? "Hide the side panel" : "Show the side panel"} pressed={layout.rightOpen} onclick={() => (layout.rightOpen = !layout.rightOpen)} />
                </Panel>
              {/if}
            </span>
          {/if}
        </div>
        <div class="scroll" use:track>{@render renderBody(w)}</div>
        <div class="veil" aria-hidden="true"></div>
      </Panel>
      {#if !narrow && !wins.full && per === 2 && i === anchor && i < n - 1 && (!moving2 || dragging)}
        <Grip label={`Resize ${titleOf(w)} and ${titleOf(wins.list[i + 1]!)}`} min={20} max={80} now={frac(i) * 100} style="right: calc(var(--cg, var(--s-5)) / -2 - var(--s-7) / 2)" onmove={(e) => dmove(i, e)} onend={dend} onkey={(e) => dkey(i, e)} onreset={() => { wins.split = 0.5; keep(); }} />
      {/if}
    </div>
  {/each}
  {#if !n}
    <div class="col" style:--p="1"><Panel radius="card" class="cv-wn"><div class="scroll"><div class="blank" data-blank><h1 style:font-family="var(--wm-font)">calternal</h1><p>Choose an app or a collection to begin.</p></div></div></Panel></div>
  {/if}
</div>
{#if beyond > 0}<button type="button" class="more" onclick={() => { anchor = Math.min(last, anchor + 1); glide(pos(anchor)); }} aria-label={`${beyond} more ${beyond === 1 ? "window" : "windows"} to the right`}>+{beyond}</button>{/if}
<Menu open={menuFor !== null} items={menu} label="Window" onpick={pick} onclose={() => (menuFor = null)} trigger={menuEl} />
</div>
{#if narrow && rightSheet}<Sheet open={info} label="Details" side="bottom" onclose={() => (info = false)}>{@render rightSheet(wins.focused, () => (info = false))}</Sheet>{/if}

<style>
  .canvas { --cg: var(--s-5); --lone-w: 800px; padding-inline: 0; box-sizing: border-box; transition: padding-inline var(--dur-morph) var(--ease-morph); position: relative; display: flex; gap: var(--cg); height: 100%; min-width: 0; overflow-x: auto; overflow-y: hidden; scrollbar-width: none; overscroll-behavior: contain; padding-inline: var(--s-3); }
  .cvw { position: relative; height: 100%; min-width: 0; }
  .canvas:not(.phone):not(.full)[data-windows="1"] { padding-inline: max(0px, calc((100% - var(--lone-w)) / 2)); }
  .canvas.moving.fl { -webkit-mask-image: linear-gradient(to right, transparent 0, #000 var(--s-8)); mask-image: linear-gradient(to right, transparent 0, #000 var(--s-8)); }
  .canvas.moving.fr { -webkit-mask-image: linear-gradient(to left, transparent 0, #000 var(--s-8)); mask-image: linear-gradient(to left, transparent 0, #000 var(--s-8)); }
  .canvas.moving.fl.fr { -webkit-mask-image: linear-gradient(to right, transparent 0, #000 var(--s-8), #000 calc(100% - var(--s-8)), transparent 100%); mask-image: linear-gradient(to right, transparent 0, #000 var(--s-8), #000 calc(100% - var(--s-8)), transparent 100%); }
  .canvas.phone { padding-inline: 0; }
  .col { position: relative; flex: 0 0 calc(var(--p) * (100% + var(--cg)) - var(--cg)); min-width: 0; height: 100%; transition: flex-basis var(--dur-morph) var(--ease-morph); }
  .canvas.resizing .col { transition: none; }
  .col.hide { flex-basis: 0; opacity: 0; pointer-events: none; margin-inline-end: calc(var(--cg) * -1); overflow: hidden; } .col.moving { z-index: 2; opacity: 0.9; transition: none; }
  :global(.cv-wn) { height: 100%; display: flex; flex-direction: column; overflow: hidden; }
  .wh { position: absolute; inset: 0 0 auto 0; z-index: 3; pointer-events: none; display: flex; align-items: center; gap: var(--s-4); padding: var(--s-4); min-width: 0; container-type: inline-size; } .wh > :global(*) { pointer-events: auto; }
  .canvas.phone :global(.cv-tp) { max-width: calc(100% - 2 * (var(--pill-h) + var(--s-4))); } .wh.lights { padding-left: 84px; } .sp { flex: 1; }
  :global(.cv-tp) { flex: 0 1 auto; min-width: 0; width: fit-content; max-width: min(24rem, 70%); } .tx { display: block; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tt { min-width: 0; display: flex; align-items: center; min-height: var(--tb-h); box-sizing: border-box; padding: 0 var(--s-6); font-size: var(--text-ui); line-height: 1.25; font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .tt.grab { cursor: grab; } .bt { display: inline-flex; gap: var(--tb-pair-gap); padding: 0 var(--tb-end); } .wr :global(.bt .ib) { width: var(--tb-h); }
  .scroll { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; padding-top: var(--bar-h); box-sizing: border-box; --sticky-top: var(--bar-h); -webkit-mask-image: linear-gradient(to bottom, transparent, #000 var(--s-3)); mask-image: linear-gradient(to bottom, transparent, #000 var(--s-3)); }
  :global(.scroll.ub) { padding-top: 0; } :global(.ub:not(.scroll)) { padding-top: var(--bar-h) !important; }
  .wh :global(.panel) > :global(.glass) { background: rgb(var(--tint) / 0.9); }
  .blank { display: grid; align-content: start; gap: var(--s-3); padding: var(--s-8) var(--s-7); } .blank h1 { margin: 0; font-size: calc(64 * var(--type-px, 1px)); line-height: 1.1; font-weight: 600; font-synthesis: none; } .blank p { margin: 0; color: var(--ink-3); font-size: var(--text-ui); }
  .veil { position: absolute; inset: 0; background: #000; opacity: 0; pointer-events: none; transition: opacity var(--dur-base) var(--ease-out); border-radius: inherit; }
  .canvas:not([data-windows="1"]) .col:not(.focus) .veil { opacity: 0.07; } :global(html[data-scheme="dark"]) .canvas:not([data-windows="1"]) .col:not(.focus) .veil { opacity: 0.2; }
  .col .wh { transition: opacity var(--dur-base) var(--ease-out); } .canvas:not([data-windows="1"]) .col:not(.focus) .wh .bp, .canvas:not([data-windows="1"]) .col:not(.focus) .wh > button { opacity: 0.6; }
  .phone .col::before { content: ""; position: absolute; inset: 0; border-radius: var(--r-card); background: #f3f1ee; } :global(html[data-scheme="dark"]) .phone .col::before { background: #1e1e20; }
  .more { position: absolute; inset-inline-end: var(--s-4); inset-block-end: var(--s-6); z-index: 6; min-height: var(--pill-h); min-width: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--paper, #fff); box-shadow: 0 1px 2px rgb(0 0 0 / 0.14), 0 4px 14px rgb(0 0 0 / 0.12); font-size: var(--text-ui); font-weight: var(--strong-weight); font-variant-numeric: tabular-nums; }
  .canvas.phone { display: block; overflow: hidden; } .phone .col { position: absolute; inset: 0; flex: none; transition: translate var(--dur-slow) var(--ease-out); translate: calc(var(--d) * 100%) 0; } .phone .col[style*="--d: -"] { translate: -20% 0; }
  .wh:not(.desk) { height: var(--bar-h); box-sizing: border-box; padding-block: 0; }
  .wh.desk :global(.cv-tp) { justify-self: center; max-width: 100%; width: auto; }
  .wh.desk { display: grid; grid-template-columns: 1fr minmax(0, auto) 1fr; height: var(--bar-h); box-sizing: border-box; padding-block: 0; } .wl { justify-self: start; display: inline-flex; align-items: center; gap: var(--s-4); } .wr { justify-self: end; display: inline-flex; align-items: center; gap: var(--s-4); }
  :global(.cv-tp) { transition: opacity var(--dur-base) var(--ease-out), translate var(--dur-base) var(--ease-out); } :global(.cv-tp.hid) { opacity: 0; translate: 0 -6px; pointer-events: none; }
  .tl { display: inline-flex; align-items: center; gap: 7px; padding: 0 var(--s-5) 0 var(--s-5); height: var(--tb-h); }
  :global(.cv-lp) .tl { padding: 0 calc(var(--tb-h) / 2 + 0.5px - 8px); }
  .tb { position: relative; flex: none; width: 16px; height: 16px; border-radius: 50%; display: grid; place-items: center; box-shadow: inset 0 0 0 0.5px rgb(0 0 0 / 0.22); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  .tb.red { background: #ff5f57; } .tb.green { background: #28c840; } .tb:active { scale: var(--press-scale); }
  .tb svg { width: 10px; height: 10px; fill: rgb(0 0 0 / 0.58); stroke: rgb(0 0 0 / 0.58); stroke-width: 1.1; stroke-linecap: round; opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); } .tb.green svg { stroke-width: 0; } .tb.red svg { fill: none; }
  .tl:hover .tb svg, .tl:focus-within .tb svg, .tb.max svg { opacity: 1; }
  .canvas:not([data-windows="1"]) .col:not(.focus) .tb { background: rgb(var(--ink-rgb) / 0.2); } .canvas:not([data-windows="1"]) .col:not(.focus) .tl:hover .tb.red { background: #ff5f57; } .canvas:not([data-windows="1"]) .col:not(.focus) .tl:hover .tb.green { background: #28c840; }
  @media (hover: none) { .tl { gap: 28px; padding: 0 var(--s-6) 0 28px; } .tb::after { content: ""; position: absolute; inset: -14px; } }
  :global(html) { --win-a: 1; } :global(html[data-winbg="frosted"]) { --win-a: 0.8; } :global(html[data-winbg="frosted"][data-scheme="dark"]) { --win-a: 0.76; }
  :global(.cv-wn) > :global(.glass) { background: rgb(var(--tint) / var(--win-a)); box-shadow: 0 2px 14px rgb(0 0 0 / 0.16), 0 1px 3px rgb(0 0 0 / 0.1); }
  :global(html[data-scheme="dark"]) :global(.cv-wn) > :global(.glass) { box-shadow: 0 2px 14px rgb(0 0 0 / 0.38), 0 1px 3px rgb(0 0 0 / 0.2); }
  :global(.shell:not(.narrow)) .canvas:not(.phone) { padding-block: var(--s-3); }
</style>
