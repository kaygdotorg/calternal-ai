<script lang="ts">
  // THE APP SWITCHER, at the canvas's bottom centre (owner's sketch, 2026-11-24):
  // one capsule holding a horizontally scrolling strip of the space's apps and the
  // person's own sidebars, with a "+" button and drag-to-reorder / Alt+Left/Right.
  // Shared verbatim from apps/mockup/src/shell/AppStrip.svelte.
  import { onMount, tick } from "svelte";
  import { Panel, tip, tokenMs } from "@calternal/ui";
  import AppIcon from "./AppIcon.svelte";

  export interface StripItem {
    id: string;
    name: string;
    icon: string;
  }

  let {
    items,
    current,
    onselect,
    onplace,
    onmove,
    onadd,
    narrow = false,
    shown = true,
    labelsMode = "auto",
    maxCount = 3,
    onfocusin,
    onfocusout,
  }: {
    items: StripItem[];
    current: string | null;
    onselect: (id: string | null) => void;
    onplace?: (id: string, index: number) => void;
    onmove?: (id: string, by: number) => void;
    onadd?: (name: string) => string | null;
    narrow?: boolean;
    shown?: boolean;
    labelsMode?: "auto" | "text" | "icons" | "both";
    maxCount?: number;
    onfocusin?: () => void;
    onfocusout?: () => void;
  } = $props();

  const mode = $derived(labelsMode === "auto" ? (narrow ? "icons" : "text") : labelsMode);
  const totalSlots = $derived(items.length + (onadd ? 1 : 0));
  const count = $derived(Math.min(narrow ? 5 : maxCount, Math.max(1, totalSlots)));
  const ITEM = $derived(mode === "icons" ? (narrow ? 44 : 36) : mode === "both" ? 112 : 92);
  let track = $state<HTMLElement>();
  let more = $state({ l: false, r: false });
  let naming = $state(false);
  let nameText = $state("");

  const edges = () => {
    if (!track) return;
    more = { l: track.scrollLeft > 2, r: track.scrollLeft + track.clientWidth < track.scrollWidth - 2 };
  };
  $effect(() => { void items.length; void mode; void count; queueMicrotask(edges); });
  $effect(() => {
    void current; void mode;
    tick().then(() => {
      const el = track?.querySelector<HTMLElement>("[aria-current=true]");
      if (!el || !track) return;
      const l = el.offsetLeft - track.offsetLeft;
      if (l < track.scrollLeft || l + el.offsetWidth > track.scrollLeft + track.clientWidth) {
        track.scrollTo({
          left: l - (track.clientWidth - el.offsetWidth) / 2,
          behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
        });
      }
    });
  });

  function nameTip(node: HTMLElement, label: string) {
    if (!matchMedia("(hover: hover)").matches) label = "";
    let t = label ? tip(node, { label }) : null;
    return {
      update(l: string) { t?.destroy?.(); t = l ? tip(node, { label: l }) : null; },
      destroy() { t?.destroy?.(); },
    };
  }

  const choose = (id: string) => { if (!quiet) onselect(current === id ? null : id); };

  let drag = $state<{ id: string; from: number; to: number; dx: number; settling: boolean } | null>(null);
  let commit = $state(false);
  let said = $state("");
  let quiet = false;

  function pdown(e: PointerEvent, id: string) {
    if (e.button !== 0 || drag || !track || !onplace) return;
    const el = e.currentTarget as HTMLElement, x0 = e.clientX, y0 = e.clientY, pid = e.pointerId, touch = e.pointerType !== "mouse";
    const from = items.findIndex((i) => i.id === id), s0 = track.scrollLeft;
    let live = false, x = x0, hold = 0, raf = 0;
    const apply = () => {
      if (!track || !drag) return;
      const dx = x - x0 + (track.scrollLeft - s0);
      const to = Math.max(0, Math.min(items.length - 1, from + Math.round(dx / ITEM)));
      drag = { ...drag, dx: Math.max(-from * ITEM, Math.min((items.length - 1 - from) * ITEM, dx)), to };
    };
    let last = 0;
    const scroll = (now: number) => {
      if (!track || !drag || drag.settling) return;
      const dt = last ? Math.min(64, now - last) : 16; last = now;
      const r = track.getBoundingClientRect();
      const v = x < r.left + 24 ? -1 : x > r.right - 24 ? 1 : 0;
      if (v) { track.scrollLeft += v * 0.36 * dt; apply(); }
      raf = requestAnimationFrame(scroll);
    };
    const start = () => {
      live = true; quiet = true; drag = { id, from, to: from, dx: 0, settling: false };
      if (touch) navigator.vibrate?.(8);
      apply(); raf = requestAnimationFrame(scroll);
    };
    const still = (t: TouchEvent) => { if (live) t.preventDefault(); };
    const off = () => {
      clearTimeout(hold); cancelAnimationFrame(raf);
      removeEventListener("pointermove", move); removeEventListener("pointerup", up);
      removeEventListener("pointercancel", cancel); removeEventListener("keydown", esc, true);
      removeEventListener("touchmove", still);
    };
    const move = (m: PointerEvent) => {
      if (m.pointerId !== pid) return;
      x = m.clientX;
      if (live) return apply();
      const far = Math.hypot(m.clientX - x0, m.clientY - y0);
      if (touch) { if (far > 8) off(); } else if (Math.abs(m.clientX - x0) > 5) start();
    };
    const settle = (to: number) => {
      if (!drag) return;
      drag = { ...drag, to, dx: (to - from) * ITEM, settling: true };
      window.setTimeout(() => {
        commit = true;
        if (to !== from) { onplace(id, to); said = `${el.getAttribute("aria-label")} moved to place ${to + 1}`; }
        drag = null;
        requestAnimationFrame(() => requestAnimationFrame(() => (commit = false)));
        window.setTimeout(() => (quiet = false), 0);
      }, matchMedia("(prefers-reduced-motion: reduce)").matches ? 1 : tokenMs("--dur-base", 220));
    };
    const up = (u: PointerEvent) => { if (u.pointerId !== pid) return; off(); if (live) settle(drag!.to); };
    const cancel = () => { off(); if (live) settle(from); };
    const esc = (k: KeyboardEvent) => { if (k.key === "Escape" && live) { k.preventDefault(); k.stopPropagation(); cancel(); } };
    addEventListener("pointermove", move); addEventListener("pointerup", up);
    addEventListener("pointercancel", cancel); addEventListener("keydown", esc, true);
    addEventListener("touchmove", still, { passive: false });
    if (touch) hold = window.setTimeout(start, 350);
  }

  const shift = (i: number) => !drag ? 0 : i === drag.from ? drag.dx : drag.from < drag.to && i > drag.from && i <= drag.to ? -ITEM : drag.to < drag.from && i >= drag.to && i < drag.from ? ITEM : 0;

  function create() {
    const n = nameText.trim(); naming = false; nameText = "";
    if (n && onadd) { const created = onadd(n); if (created) onselect(created); }
  }

  const key = (e: KeyboardEvent) => {
    if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight") && onmove) {
      const b = (e.target as HTMLElement).closest<HTMLElement>("[data-app]");
      if (!b) return;
      e.preventDefault();
      const id = b.dataset.app!;
      onmove(id, e.key === "ArrowLeft" ? -1 : 1);
      said = `${b.getAttribute("aria-label")} moved`;
      tick().then(() => track?.querySelector<HTMLElement>(`[data-app="${id}"]`)?.focus());
      return;
    }
    const bs = [...(track?.querySelectorAll<HTMLElement>("button") ?? [])];
    const i = bs.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault();
      bs[Math.max(0, Math.min(bs.length - 1, i + (e.key === "ArrowRight" ? 1 : -1)))]?.focus();
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      bs[e.key === "Home" ? 0 : bs.length - 1]?.focus();
    }
  };

  onMount(() => {
    const ro = track ? new ResizeObserver(edges) : null;
    if (track) ro?.observe(track);
    return () => ro?.disconnect();
  });
</script>

<div class="dock" class:narrow data-shown={shown} data-switcher-dock>
  <Panel radius="pill" class="sw-pill" data-switcher role="toolbar" aria-label="Apps" onfocusin={() => onfocusin?.()} onfocusout={(e: FocusEvent) => { if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node | null)) onfocusout?.(); }} onkeydown={key}>
    {#if naming}
      <form class="nm" onsubmit={(e) => { e.preventDefault(); create(); }}>
        <input bind:value={nameText} placeholder="Name the sidebar" aria-label="Sidebar name" onblur={create} onkeydown={(e) => e.key === "Escape" && ((nameText = ""), (naming = false))} />
      </form>
    {:else}
      <div class="track" class:ml={more.l} class:mr={more.r} class:sorting={!!drag} class:commit bind:this={track} onscroll={edges} style:--w="{ITEM}px" style:--n={count} role="list">
        {#each items as it, i (it.id)}
          <div role="listitem" class="sl" class:held={drag?.id === it.id} class:settling={drag?.settling && drag.id === it.id} style:translate="{shift(i)}px 0">
            <button type="button" class="it" data-app={it.id} aria-keyshortcuts="Alt+ArrowLeft Alt+ArrowRight" class:on={current === it.id} class:ic={mode === "icons"} aria-current={current === it.id ? "true" : undefined} aria-label={it.name} onclick={() => choose(it.id)} onpointerdown={(e) => pdown(e, it.id)} use:nameTip={narrow ? "" : mode === "icons" ? it.name : "Ctrl or ⌘ click: new window"}>
              {#if mode !== "text"}<AppIcon name={it.icon} inline />{/if}{#if mode !== "icons"}<span class="lb">{it.name}</span>{/if}
            </button>
          </div>
        {/each}
        {#if onadd}
          <div role="listitem">
            <button type="button" class="it ic add" aria-label="New sidebar" onclick={() => { naming = true; tick().then(() => document.querySelector<HTMLInputElement>("[data-switcher] .nm input")?.focus()); }} use:tip={{ label: "New sidebar" }}>
              <AppIcon name="plus" inline />
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </Panel>
  <span class="sr" aria-live="polite">{said}</span>
</div>

<style>
  .dock { position: absolute; left: 0; right: 0; bottom: var(--s-5); z-index: 5; display: flex; justify-content: center; pointer-events: none; }
  .dock.narrow { position: fixed; bottom: calc(var(--s-3) + env(safe-area-inset-bottom, 0px) + var(--tb-search-h) + var(--s-4)); }
  .dock > :global(*) { pointer-events: auto; }
  :global(.sw-pill) { --n: 3; padding: 3px; max-width: calc(100% - var(--s-7)); transition: translate var(--dur-leave) var(--ease-out), opacity var(--dur-leave) var(--ease-out); }
  .dock[data-shown="false"] :global(.sw-pill) { translate: 0 calc(100% + var(--s-6)); opacity: 0; pointer-events: none; }
  .dock[data-shown="true"] :global(.sw-pill) { transition: translate var(--dur-base) var(--ease-out) 70ms, opacity var(--dur-base) var(--ease-out) 70ms; }
  .track { margin-block: -3px; padding-block: 3px; display: flex; width: calc(var(--n) * var(--w)); max-width: 100%; overflow-x: auto; scroll-snap-type: x mandatory; scrollbar-width: none; overscroll-behavior-x: contain; touch-action: pan-x; border-radius: var(--r-pill); }
  .track.mr { -webkit-mask-image: linear-gradient(to right, #000 calc(100% - var(--s-7)), transparent); mask-image: linear-gradient(to right, #000 calc(100% - var(--s-7)), transparent); }
  .track.ml { -webkit-mask-image: linear-gradient(to right, transparent, #000 var(--s-7)); mask-image: linear-gradient(to right, transparent, #000 var(--s-7)); }
  .track.ml.mr { -webkit-mask-image: linear-gradient(to right, transparent, #000 var(--s-7), #000 calc(100% - var(--s-7)), transparent); mask-image: linear-gradient(to right, transparent, #000 var(--s-7), #000 calc(100% - var(--s-7)), transparent); }
  .track > [role=listitem] { flex: none; width: var(--w); scroll-snap-align: start; }
  .it { width: 100%; height: calc(var(--tb-h) - 6px); display: inline-flex; align-items: center; justify-content: center; gap: var(--s-3); padding: 0 var(--s-4); border-radius: var(--r-pill); color: var(--ink-2); font-size: var(--text-ui); font-weight: var(--ui-weight); white-space: nowrap; transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  .it .lb { overflow: hidden; text-overflow: ellipsis; } .it:active { scale: var(--press-scale); }
  .it.on { background: var(--accent-soft); color: var(--ink); font-weight: var(--strong-weight); } .it.on :global(svg) { color: var(--accent); }
  @media (hover: hover) and (pointer: fine) { .it:not(.on):hover { background: var(--hover); color: var(--ink); } }
  .it:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .nm { padding: 0 var(--s-4); width: calc(var(--s-10) * 4); } .nm input { width: 100%; height: calc(var(--tb-h) - 6px); background: none; border: 0; outline: 0; color: var(--ink); font: inherit; font-size: var(--text-ui); }
  .sl { transition: translate var(--dur-base) var(--ease-out); } .track.commit .sl { transition: none; } .sl .it { touch-action: pan-x; -webkit-user-select: none; user-select: none; -webkit-touch-callout: none; transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out); }
  .sl.held { position: relative; z-index: 2; transition: none; } .sl.held.settling { transition: translate var(--dur-base) var(--ease-out); } .sl.held:not(.settling) .it { scale: 1.05; background: var(--paper, #fff); box-shadow: 0 1px 3px rgb(0 0 0 / 0.3); cursor: grabbing; } :global(html[data-scheme="dark"]) .sl.held:not(.settling) .it { background: #3a3a3d; }
  .track.sorting { scroll-snap-type: none; -webkit-mask-image: none; mask-image: none; cursor: grabbing; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
</style>
