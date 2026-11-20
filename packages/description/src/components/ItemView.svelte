<script lang="ts">
  // The `item-view` host component (docs/description-contract.md 5.1, 5.2 and "Entry views"): the retained or live body of the item the config names, read with `item_view.open` (the newest page, or the first unread
  // entry when the view declares reading counts) and kept current through `item_view.follow` (changed: fetch the newest head; reader_state: only the private overlays changed; invalidated: clear what is drawn). A conversation draws
  // its entries with the host's overlays: own flag and provider mark, quotes, request cards, activity, access (read-only and why). The reading position is written with `item_view.read` only for an entry that has
  // scrolled into view; a "jump to latest" control carries the unread count and an "@" control the mention count. Entry actions come from the config and the host's per-entry contexts (a menu on the entry); an action may
  // open the composer in reply or edit mode; with `selectionMode: "range"` the person picks first and last message and the host returns a selection ref that range actions send. A stale result is marked; moving to
  // another item cancels the old load; the view follows the end while the person reads the end and never pulls a person who scrolled away.
  import { onDestroy, tick } from "svelte";
  import { Menu, Spinner, failed, tokenMs } from "@calternal/ui";
  import type { ActionOutcome } from "../model";
  import type { EntryAction, ItemViewConfig, ItemViewReply, ItemViewSignal, SnapshotNode } from "../draft";
  import { useApp, useActions } from "../invoke/context";
  import { resolveRef } from "./ref";
  import { provideEntry } from "./entry-context";
  import { ItemViewState } from "./entry-state.svelte";
  import Snapshot from "./Snapshot.svelte";

  let { config, id }: { config: ItemViewConfig; id: string } = $props();
  const app = useApp(), { session, ui, viewContext } = useActions();
  const item = $derived(resolveRef(config.item, app));
  const st = new ItemViewState();
  app.itemViews.map.set(id, st);
  onDestroy(() => { app.itemViews.map.delete(id); });
  const reply = $derived(st.reply);
  let failedOpen = $state(false), reconnecting = $state(false), el = $state<HTMLElement>(), away = $state(false), back = $state<number | null>(null);
  let menu = $state<{ anchor: string; x: number; y: number } | null>(null);
  let atEnd = true, generation = 0, nudge: (() => void) | null = null, lastMarked = "", markTimer = 0, reopenWith: { anchor?: string; position?: string } | null = null;
  const reading = $derived(!!config.reading);

  const anchorsOf = (n: SnapshotNode | undefined, out: string[] = []): string[] => { if (n?.type === "snapshot-entry") out.push(n.anchor); else if (n?.type === "stack") n.children.forEach((c) => anchorsOf(c, out)); return out; };
  /** What the newest page changed for the eye: entries that were not there before ease in once; an unread separator whose messages are now read leaves instead of vanishing. */
  function noteChanges(prev: ItemViewReply | null, next: ItemViewReply) {
    if (prev?.view) {
      const had = new Set(anchorsOf(prev.view.content)), fresh = anchorsOf(next.view?.content).filter((a) => !had.has(a));
      if (fresh.length && had.size) { st.fresh = new Set(fresh); setTimeout(() => { st.fresh = new Set(); }, tokenMs("--dur-base", 220) + 80); }
      const was = prev.reading?.first_unread;
      if (was && next.reading?.first_unread !== was) leave(was);
    }
  }
  function leave(anchor: string) { st.leaving = anchor; setTimeout(() => { if (st.leaving === anchor) st.leaving = null; }, tokenMs("--dur-slow", 300) + 80); }
  type Body = { anchor?: string; position?: "unread" | "latest" | "mention"; cursor?: string };
  async function open(it: string, ctl: AbortController, more: Body = {}): Promise<ItemViewReply | null> {
    try { return await session.exchange<ItemViewReply>("item_view.open", { type: "item_view", item: it, component: id, slot: config.slot, live: true, ...more }, ctl.signal); } catch { return null; }
  }
  const children = (n: SnapshotNode | undefined): SnapshotNode[] => (n?.type === "stack" ? n.children : n ? [n] : []);

  $effect(() => {
    const it = item; const gen = ++generation; const ctl = new AbortController();
    st.reply = null; st.composer = null; st.endSelection(); failedOpen = false; reconnecting = false; lastMarked = ""; away = false;
    if (!it) return;
    void (async () => {
      let first = true, retry = 0;
      let r = await open(it, ctl, reading ? { position: "unread" } : {});
      while (gen === generation && !ctl.signal.aborted) {
        // The connection dropped (a server restart, a network gap) while a run may continue: nothing is failed. What is drawn stays, a quiet "Reconnecting" says so, and the view asks again with a growing pause (1 s to 15 s)
        // until the host answers; then the page it returns carries whatever the run wrote meanwhile, and the new messages ease in.
        if (!r) { reconnecting = true; await new Promise((res) => setTimeout(res, Math.min(15000, 1000 * 2 ** retry++))); if (gen !== generation || ctl.signal.aborted) return; r = await open(it, ctl); continue; }
        retry = 0; reconnecting = false;
        if (r.state === "anchor_unavailable") { r = await open(it, ctl); continue; }
        const prev = st.reply; st.reply = r; failedOpen = false; noteChanges(prev, r); await tick();
        if (first) { first = false; if (reading && r.reading?.first_unread && el?.querySelector(`[data-anchor="${CSS.escape(r.reading.first_unread)}"]`)) scrollTo(r.reading.first_unread, "start"); else if (r.anchor) scrollTo(r.anchor, "center"); else if (el) el.scrollTop = el.scrollHeight; }
        else if (atEnd && el) el.scrollTop = el.scrollHeight;
        watch();
        if (!r.follow) return;
        let sig: ItemViewSignal | null = null;
        const fctl = new AbortController(); ctl.signal.addEventListener("abort", () => fctl.abort(), { once: true }); nudge = () => fctl.abort();
        try { sig = await session.exchange<ItemViewSignal>("item_view.follow", { type: "item_view_follow", follow: r.follow }, fctl.signal); } catch { sig = null; }
        nudge = null;
        if (gen !== generation || ctl.signal.aborted) return;
        if (sig?.state === "invalidated") st.reply = null;
        if (sig?.state === "closed") return;
        const w = reopenWith; reopenWith = null;
        r = await open(it, ctl, w ? { ...(w.anchor ? { anchor: w.anchor } : {}), ...(w.position ? { position: w.position as Body["position"] } : {}) } : {});
        if (w && r?.anchor) { first = true; }
      }
    })();
    return () => { ctl.abort(); clearTimeout(markTimer); io?.disconnect(); };
  });

  function onscroll() { if (!el) return; atEnd = el.scrollHeight - el.scrollTop - el.clientHeight < 40; away = !atEnd; }
  function scrollTo(anchor: string, block: "start" | "center") {
    const t = el?.querySelector<HTMLElement>(`[data-anchor="${CSS.escape(anchor)}"]`); if (!t) return false;
    t.scrollIntoView({ block, behavior: "auto" }); t.classList.remove("hl"); void t.offsetWidth; t.classList.add("hl"); return true;
  }
  /** Jump to an anchor: on the page, or by opening the view at that anchor; a "back" chip returns to where the person was. */
  function jump(anchor: string) {
    if (el) back = el.scrollTop;
    if (!scrollTo(anchor, "center")) { reopenWith = { anchor }; nudge?.(); }
    setTimeout(() => { back = null; }, 8000);
  }
  /** Jump to latest: the scroll position eases to the end over --dur-slow on a cubic ease-out (instant when motion is reduced); any wheel, touch or key from the person takes it over. */
  let tween = 0;
  function toLatest() {
    atEnd = true; away = false; if (!el) return;
    const box = el, from = box.scrollTop, ms = tokenMs("--dur-slow", 300), t0 = performance.now(); cancelAnimationFrame(tween);
    const stop = () => { cancelAnimationFrame(tween); box.removeEventListener("wheel", stop); box.removeEventListener("touchstart", stop); box.removeEventListener("keydown", stop); };
    box.addEventListener("wheel", stop, { once: true, passive: true }); box.addEventListener("touchstart", stop, { once: true, passive: true }); box.addEventListener("keydown", stop, { once: true });
    const step = (now: number) => { const k = ms <= 1 ? 1 : Math.min(1, (now - t0) / ms), e = 1 - (1 - k) ** 3; box.scrollTop = from + (box.scrollHeight - box.clientHeight - from) * e; if (k < 1) tween = requestAnimationFrame(step); else stop(); };
    tween = requestAnimationFrame(step);
  }
  function nextMention() { const a = reply?.reading?.next_mention; if (a) jump(a); else { reopenWith = { position: "mention" }; nudge?.(); } }

  // ---- reading position: the last entry that has scrolled into view, written once the view settles (never moved backwards by the host)
  let io: IntersectionObserver | undefined;
  const visible = new Set<Element>();
  function watch() {
    io?.disconnect(); visible.clear();
    if (!reading || !el) return;
    io = new IntersectionObserver((es) => {
      for (const e of es) { if (e.isIntersecting) visible.add(e.target); else visible.delete(e.target); }
      clearTimeout(markTimer); markTimer = window.setTimeout(mark, 700);
    }, { root: el, threshold: 0.6 });
    for (const n of el.querySelectorAll("[data-entry]")) io.observe(n);
  }
  async function mark() {
    if (!el || !visible.size || !reply?.window_ref) return;
    const all = [...el.querySelectorAll("[data-entry]")];
    const last = [...visible].sort((a, b) => all.indexOf(b) - all.indexOf(a))[0] as HTMLElement; const upto = last?.dataset.anchor;
    if (!upto || upto === lastMarked) return;
    if (!(reply.reading?.unread || reply.reading?.mentions)) { lastMarked = upto; return; } // nothing unread: nothing to write
    lastMarked = upto;
    try { const r = await session.exchange<NonNullable<ItemViewReply["reading"]>>("item_view.read", { type: "item_view_read_mark", window_ref: reply.window_ref, upto, request_id: crypto.randomUUID() }); if (st.reply) { const was = st.reply.reading?.first_unread; st.reply = { ...st.reply, reading: r }; if (was && r.first_unread !== was) leave(was); } } catch { lastMarked = ""; }
  }

  // ---- entry actions
  const actions = $derived(config.entry_actions ?? []);
  const entryActions = $derived(actions.filter((a) => !usesSelection(a)));
  const rangeActions = $derived(actions.filter(usesSelection));
  function usesSelection(a: EntryAction) { return Object.values(a.invoke.inputs ?? {}).some((v) => (v as { entry?: string }).entry === "selection"); }
  const menuItems = $derived(menu ? [...(st.controlOf(menu.anchor)?.actions ?? []).map((x) => { const d = entryActions.find((a) => a.invoke.id === x.action)?.invoke; return { id: x.action, label: d?.title ?? x.action, danger: d?.role === "destructive" }; }),
    ...(rangeActions.length && !st.readOnly ? [{ id: "\u0000select", label: "Select messages", danger: false }] : [])] : []);
  async function pickAction(actionId: string) {
    const anchor = menu?.anchor; menu = null;
    if (actionId === "\u0000select") { st.selecting = true; if (anchor) select([anchor]); return; }
    const ctl = anchor ? st.controlOf(anchor)?.actions?.find((x) => x.action === actionId) : undefined, def = entryActions.find((a) => a.invoke.id === actionId)?.invoke;
    if (!ctl || !def) return;
    await runAction(def, ctl.activation_context);
  }
  async function runAction(def: EntryAction["invoke"], context: string | undefined, interaction?: { selection_ref?: string }) {
    const out = await ui.invoker.run(def, { context, interaction: interaction as never });
    if ("state" in out && out.state === "ok") {
      const comp = (out as ActionOutcome & { entry_composer?: string }).entry_composer;
      if ((def.then as { composer?: unknown } | undefined)?.composer && comp) {
        try { const c = await session.exchange<{ mode: "reply" | "edit"; anchor: string; initial?: string }>("entry_composer.open", { type: "entry_composer", composer: comp }); st.composer = { mode: c.mode, anchor: c.anchor, initial: c.initial, ref: comp }; }
        catch { failed("The composer could not be opened."); }
      }
      nudge?.(); st.endSelection();
    }
  }

  // ---- range selection (call an agent in on a message, a range or the whole thread)
  /** Picks a message, or a range: a click on the first picks it, a second picks the last; a third starts again. The host issues the reference for the range (the page never names bytes or authors). */
  let selTimer = 0;
  function pick(anchor: string) { select(st.picked.length === 1 && st.origin ? [st.origin, anchor] : [anchor]); }
  function select(p: string[]) {
    st.origin = p[0]!;
    const all = [...(el?.querySelectorAll("[data-entry]") ?? [])].map((e) => (e as HTMLElement).dataset.anchor!);
    const [a, b] = p.length === 2 ? [...p].sort((x, y) => all.indexOf(x) - all.indexOf(y)) : [p[0]!, undefined];
    st.picked = b && b !== a ? [a, b] : [a]; st.selectionRef = undefined; st.count = b ? all.indexOf(b) - all.indexOf(a) + 1 : 1;
    clearTimeout(selTimer);
    selTimer = window.setTimeout(async () => {
      if (!reply?.window_ref) return;
      try { const r = await session.exchange<{ selection_ref: string }>("item_view.selection", { type: "item_view_selection", window_ref: reply.window_ref, first: a, ...(b && b !== a ? { last: b } : {}) }); st.selectionRef = r.selection_ref; }
      catch { failed("Those messages could not be selected."); st.picked = []; st.count = 0; }
    }, 200);
  }
  async function openLink(target: { space: string; id: string }) {
    try { const r = await session.exchange<{ item: string }>("item.locate", { type: "item_locate", locator: target }); app.setRoute("item", r.item); }
    catch { failed("That link is not available to you."); }
  }
  provideEntry({ state: st, menu: (anchor, at) => { menu = { anchor, ...at }; }, pick, jump, link: (t) => void openLink(t) });

  // ---- keyboard: Up, Down, Home and End move between messages (one tab stop for the whole log); Enter or the Menu key opens the message menu; Shift with an arrow starts a selection and extends it, Space picks, Escape ends it.
  const entryEls = () => [...(el?.querySelectorAll<HTMLElement>("[data-entry]") ?? [])];
  function focusEntry(n: HTMLElement) { st.active = n.dataset.anchor!; n.focus({ preventScroll: true }); n.scrollIntoView({ block: "nearest" }); }
  function keydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("button, a, input, textarea")) return;
    const list = entryEls(); if (!list.length) return;
    const i = list.findIndex((x) => x.dataset.anchor === st.active), last = list.length - 1;
    const to = e.key === "ArrowDown" ? Math.min(last, i < 0 ? last : i + 1) : e.key === "ArrowUp" ? Math.max(0, i < 0 ? last : i - 1) : e.key === "Home" ? 0 : e.key === "End" ? last : -1;
    if (to >= 0) {
      e.preventDefault(); const from = i < 0 ? to : i; focusEntry(list[to]!);
      if (e.shiftKey && rangeActions.length && !st.readOnly) { if (!st.selecting) st.selecting = true; select([st.origin ?? list[from]!.dataset.anchor!, list[to]!.dataset.anchor!]); }
    } else if ((e.key === "Enter" || e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) && i >= 0 && !st.selecting) {
      e.preventDefault(); const r = list[i]!.getBoundingClientRect(); if (st.controlOf(st.active!)?.actions?.length) menu = { anchor: st.active!, x: r.left + 24, y: r.top + 24 };
    } else if (e.key === " " && st.selecting && i >= 0) { e.preventDefault(); pick(st.active!); }
    else if (e.key === "Escape" && st.selecting) { e.preventDefault(); st.endSelection(); }
  }

  // ---- live activity: a row per entry, dropped locally at its expiry
  let now = $state(Date.now()); const tickH = setInterval(() => { now = Date.now(); }, 1000); onDestroy(() => clearInterval(tickH));
  const activity = $derived((reply?.activity ?? []).filter((a) => new Date(a.expires_at).getTime() > now));
  const reasonText = { lapsed: "You no longer share a group with everyone in this thread. The history stays, but nobody can write in it.", archived: "This thread is archived. Unarchive it to write in it.", trashed: "This thread is in the Trash.", grant: "You can read this thread but not write in it." } as const;
</script>

<section class="iv" data-host-component="item-view" data-item={item} data-state={failedOpen ? "error" : reply?.state ?? (item ? "loading" : "idle")} aria-label="Conversation">
  {#if !item}<p class="msg" data-view-state="idle">Select a thread, or start one.</p>
  {:else if failedOpen}<p class="msg bad" role="alert" data-view-state="error">This thread could not be opened. It will be tried again when it changes.</p>
  {:else if reconnecting && !reply?.view}<p class="msg" role="status" data-view-state="reconnecting"><Spinner /> Reconnecting… your agent keeps running. The thread catches up when the connection is back.</p>
  {:else if reply?.state === "unavailable"}<p class="msg" data-view-state="unavailable">This thread is not available to you.</p>
  {:else if !reply || reply.state === "loading" || !reply.view}<p class="msg" role="status" data-view-state="loading">Opening the thread…</p>
  {:else}
    {#if reconnecting}<p class="note rc" role="status" data-view-state="reconnecting"><Spinner /> Reconnecting… your agent keeps running. The thread catches up when the connection is back.</p>{/if}
    {#if reply.state === "stale"}<p class="note" data-view-state="stale">Showing an earlier version. It will update when the thread wakes.</p>{/if}
    {#if st.readOnly}<p class="note ro" role="status" data-access="read_only" data-reason={st.readOnly.reason}>{reasonText[st.readOnly.reason]}</p>{/if}
    {#if st.selecting}
      <div class="bar" data-selection-bar role="toolbar" aria-label="Select messages">
        <button type="button" data-select-cancel onclick={() => st.endSelection()}>Cancel</button>
        <span class="cnt" aria-live="polite">{st.picked.length === 0 ? "Choose the first message" : st.picked.length === 1 ? "1 message selected. Choose the last, or use just this one" : `${st.count} messages selected`}</span>
        {#each rangeActions as a (a.invoke.id)}<button type="button" class="p" data-range-action={a.invoke.id} disabled={!st.selectionRef} onclick={() => runAction(a.invoke, viewContext(a.invoke.id), { selection_ref: st.selectionRef })}>{a.invoke.title}</button>{/each}
      </div>
    {/if}
    <div class="wrap">
      <div class="scroll" bind:this={el} {onscroll} onkeydown={keydown} tabindex={st.active ? -1 : 0} role={st.selecting ? "listbox" : "log"} aria-multiselectable={st.selecting || undefined} aria-live={st.selecting ? "off" : "polite"} aria-label="Messages">
        {#if reply.older}<button type="button" class="older" data-older onclick={async () => { const ctl = new AbortController(); const r = await open(item!, ctl, { cursor: reply!.older }); if (r?.view && st.reply) { st.reply = { ...st.reply, older: r.older, view: { content: { type: "stack", children: [...children(r.view.content), ...children(st.reply.view?.content)] } } }; } }}>Earlier messages</button>{/if}
        <Snapshot node={reply.view.content} />
        {#each activity as a (a.label + a.kind)}<p class="act" data-activity={a.kind} role="status">{a.label}…</p>{/each}
      </div>
      {#if back !== null}<button type="button" class="chip" data-back-chip onclick={() => { if (el) el.scrollTop = back!; back = null; }}>Back to where you were</button>{/if}
      {#if reading && (away || reply.reading?.unread)}
        <div class="jumps">
          {#if reply.reading?.mentions}<button type="button" class="jb" data-jump-mention aria-label={`${reply.reading.mentions} unread mentions`} onclick={nextMention}>@<span class="n">{reply.reading.mentions}</span></button>{/if}
          {#if away}<button type="button" class="jb" data-jump-latest aria-label={reply.reading?.unread ? `Jump to latest, ${reply.reading.unread} unread` : "Jump to latest"} onclick={toLatest}>↓{#if reply.reading?.unread}<span class="n">{reply.reading.unread}</span>{/if}</button>{/if}
        </div>
      {/if}
    </div>
    <Menu open={!!menu} items={menuItems} label="Message actions" onpick={pickAction} onclose={() => (menu = null)} trigger={null} anchor={menu ? { left: menu.x, top: menu.y, right: menu.x, bottom: menu.y } : null} />
  {/if}
</section>

<style>
  .iv { display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .wrap { position: relative; display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .scroll { overflow-y: auto; flex: 1; min-height: 0; padding: var(--s-2) var(--s-4) var(--s-4); display: flex; flex-direction: column; gap: var(--s-1); overscroll-behavior: contain; }
  .scroll > :global(.s) { gap: var(--s-1); }
  .msg { margin: auto; color: var(--ink-3); text-align: center; padding: var(--s-6); } .msg.bad { color: var(--danger); }
  .note { flex: none; margin: 0 var(--s-4) var(--s-1); padding: var(--s-2) var(--s-4); border-radius: var(--r-select); color: var(--warn); font-size: var(--text-label); } .note.ro { color: var(--ink-2); background: rgb(var(--ink-rgb) / 0.07); }
  .bar { flex: none; display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3); margin: 0 var(--s-4) var(--s-2); padding: var(--s-1) var(--s-2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); }
  .bar button, .older, .chip { min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); color: var(--ink); font-size: var(--text-ui); font-weight: var(--strong-weight); } .bar .p { background: var(--accent-base); color: #fff; margin-inline-start: auto; } .bar button:disabled { opacity: 0.4; }
  .cnt { color: var(--ink-2); font-size: var(--text-label); } .older { align-self: center; margin-bottom: var(--s-3); background: rgb(var(--ink-rgb) / 0.07); }
  .chip { position: absolute; top: var(--s-3); left: 50%; translate: -50% 0; background: rgb(var(--tint) / 0.9); box-shadow: var(--shadow-float); animation: rise var(--dur-base) var(--ease-out); }
  .jumps { position: absolute; right: var(--s-5); bottom: var(--s-4); display: grid; gap: var(--s-3); }
  .jb { position: relative; display: grid; place-items: center; width: var(--pill-h); height: var(--pill-h); border-radius: 50%; background: rgb(var(--tint) / 0.9); color: var(--ink); font-size: var(--text-ui); font-weight: var(--strong-weight); box-shadow: var(--shadow-float); animation: rise var(--dur-base) var(--ease-out); transition: scale var(--dur-fast) var(--ease-out); }
  .jb:active { scale: var(--press-scale); }
  .jb .n { position: absolute; top: -6px; right: -6px; min-width: 20px; height: 20px; padding: 0 6px; box-sizing: border-box; border-radius: 10px; background: var(--accent-base); color: #fff; font-size: var(--text-label); display: grid; place-items: center; }
  .act { margin: var(--s-3) var(--s-4) 0; color: var(--ink-3); font-size: var(--text-label); }
  @keyframes rise { from { opacity: 0; translate: 0 var(--s-3); } to { opacity: 1; translate: 0 0; } }
</style>
