<script lang="ts">
  // The `item-view` host component (docs/description-contract.md 5.1, 5.2 and "Entry views"): the retained or live body of the item the config names, read with `item_view.open` (the newest page, or the first unread
  // entry when the view declares reading counts) and kept current through `item_view.follow` (changed: fetch the newest head; reader_state: only the private overlays changed; invalidated: clear what is drawn). A conversation draws
  // its entries with the host's overlays: own flag and provider mark, quotes, request cards, activity, access (read-only and why). The reading position is written with `item_view.read` only for an entry that has
  // scrolled into view; a "jump to latest" control carries the unread count and an "@" control the mention count. Entry actions come from the config and the host's per-entry contexts (a menu on the entry); an action may
  // open the composer in reply or edit mode; with `selectionMode: "range"` the person picks first and last message and the host returns a selection ref that range actions send. A stale result is marked; moving to
  // another item cancels the old load; the view follows the end while the person reads the end and never pulls a person who scrolled away.
  import { onDestroy, tick } from "svelte";
  import { Menu, failed } from "@calternal/ui";
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
  let failedOpen = $state(false), el = $state<HTMLElement>(), away = $state(false), back = $state<number | null>(null);
  let menu = $state<{ anchor: string; x: number; y: number } | null>(null);
  let atEnd = true, generation = 0, nudge: (() => void) | null = null, lastMarked = "", markTimer = 0, reopenWith: { anchor?: string; position?: string } | null = null;
  const reading = $derived(!!config.reading);

  type Body = { anchor?: string; position?: "unread" | "latest" | "mention"; cursor?: string };
  async function open(it: string, ctl: AbortController, more: Body = {}): Promise<ItemViewReply | null> {
    try { return await session.exchange<ItemViewReply>("item_view.open", { type: "item_view", item: it, component: id, slot: config.slot, live: true, ...more }, ctl.signal); } catch { return null; }
  }
  const children = (n: SnapshotNode | undefined): SnapshotNode[] => (n?.type === "stack" ? n.children : n ? [n] : []);

  $effect(() => {
    const it = item; const gen = ++generation; const ctl = new AbortController();
    st.reply = null; st.composer = null; st.endSelection(); failedOpen = false; lastMarked = ""; away = false;
    if (!it) return;
    void (async () => {
      let first = true;
      let r = await open(it, ctl, reading ? { position: "unread" } : {});
      while (gen === generation && !ctl.signal.aborted) {
        if (!r) { failedOpen = true; return; }
        if (r.state === "anchor_unavailable") { r = await open(it, ctl); continue; }
        st.reply = r; failedOpen = false; await tick();
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
  function toLatest() { atEnd = true; away = false; if (el) el.scrollTo({ top: el.scrollHeight, behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" }); }
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
    try { const r = await session.exchange<NonNullable<ItemViewReply["reading"]>>("item_view.read", { type: "item_view_read_mark", window_ref: reply.window_ref, upto, request_id: crypto.randomUUID() }); if (st.reply) st.reply = { ...st.reply, reading: r }; } catch { lastMarked = ""; }
  }

  // ---- entry actions
  const actions = $derived(config.entry_actions ?? []);
  const entryActions = $derived(actions.filter((a) => !usesSelection(a)));
  const rangeActions = $derived(actions.filter(usesSelection));
  function usesSelection(a: EntryAction) { return Object.values(a.invoke.inputs ?? {}).some((v) => (v as { entry?: string }).entry === "selection"); }
  const menuItems = $derived(menu ? (st.controlOf(menu.anchor)?.actions ?? []).map((x) => { const d = entryActions.find((a) => a.invoke.id === x.action)?.invoke; return { id: x.action, label: d?.title ?? x.action, danger: d?.role === "destructive" }; }) : []);
  async function pickAction(actionId: string) {
    const anchor = menu?.anchor; menu = null;
    const ctl = anchor ? st.controlOf(anchor)?.actions?.find((x) => x.action === actionId) : undefined, def = entryActions.find((a) => a.invoke.id === actionId)?.invoke;
    if (!ctl || !def) return;
    await runAction(def, ctl.activation_context);
  }
  async function runAction(def: EntryAction["invoke"], context: string | undefined, interaction?: { selection_ref?: string }) {
    const out = await ui.invoker.run(def, { context, interaction: interaction as never });
    if ("state" in out && out.state === "ok") {
      const comp = (out as ActionOutcome & { composer?: string }).composer ?? (out as ActionOutcome).created;
      if ((def.then as { composer?: unknown } | undefined)?.composer && comp) {
        try { const c = await session.exchange<{ mode: "reply" | "edit"; anchor: string; initial?: string }>("entry_composer.open", { type: "entry_composer", composer: comp }); st.composer = { mode: c.mode, anchor: c.anchor, initial: c.initial, ref: comp }; }
        catch { failed("The composer could not be opened."); }
      }
      nudge?.(); st.endSelection();
    }
  }

  // ---- range selection (call an agent in on a message, a range or the whole thread)
  async function pick(anchor: string) {
    const p = [...st.picked, anchor];
    if (p.length > 2) { st.picked = [anchor]; st.selectionRef = undefined; return; }
    st.picked = p;
    if (!reply?.window_ref) return;
    const all = [...(el?.querySelectorAll("[data-entry]") ?? [])].map((e) => (e as HTMLElement).dataset.anchor!);
    const [a, b] = p.length === 2 ? [p[0]!, p[1]!].sort((x, y) => all.indexOf(x) - all.indexOf(y)) : [p[0]!, undefined];
    try { const r = await session.exchange<{ selection_ref: string }>("item_view.selection", { type: "item_view_selection", window_ref: reply.window_ref, first: a, ...(b && b !== a ? { last: b } : {}) }); st.selectionRef = r.selection_ref; if (b) st.picked = [a, b]; }
    catch { failed("Those messages could not be selected."); st.picked = []; }
  }
  async function openLink(target: { space: string; id: string }) {
    try { const r = await session.exchange<{ item: string }>("item.locate", { type: "item_locate", locator: target }); app.setRoute("item", r.item); }
    catch { failed("That link is not available to you."); }
  }
  provideEntry({ state: st, menu: (anchor, at) => { menu = { anchor, ...at }; }, pick: (a) => void pick(a), jump, link: (t) => void openLink(t) });

  // ---- live activity: a row per entry, dropped locally at its expiry
  let now = $state(Date.now()); const tickH = setInterval(() => { now = Date.now(); }, 1000); onDestroy(() => clearInterval(tickH));
  const activity = $derived((reply?.activity ?? []).filter((a) => new Date(a.expires_at).getTime() > now));
  const reasonText = { lapsed: "You no longer share a group with everyone in this thread. The history stays, but nobody can write in it.", archived: "This thread is archived. Unarchive it to write in it.", trashed: "This thread is in the Trash.", grant: "You can read this thread but not write in it." } as const;
</script>

<section class="iv" data-host-component="item-view" data-item={item} data-state={failedOpen ? "error" : reply?.state ?? (item ? "loading" : "idle")} aria-label="Conversation">
  {#if !item}<p class="msg" data-view-state="idle">Select a thread, or start one.</p>
  {:else if failedOpen}<p class="msg bad" role="alert" data-view-state="error">This thread could not be opened. It will be tried again when it changes.</p>
  {:else if reply?.state === "unavailable"}<p class="msg" data-view-state="unavailable">This thread is not available to you.</p>
  {:else if !reply || reply.state === "loading" || !reply.view}<p class="msg" role="status" data-view-state="loading">Opening the thread…</p>
  {:else}
    {#if reply.state === "stale"}<p class="note" data-view-state="stale">Showing an earlier version. It will update when the thread wakes.</p>{/if}
    {#if st.readOnly}<p class="note ro" role="status" data-access="read_only" data-reason={st.readOnly.reason}>{reasonText[st.readOnly.reason]}</p>{/if}
    {#if rangeActions.length && !st.readOnly}
      <div class="bar" data-selection-bar>
        {#if !st.selecting}<button type="button" data-select-start onclick={() => { st.selecting = true; }}>Select messages</button>
        {:else}<span class="cnt" aria-live="polite">{st.picked.length === 0 ? "Tap the first message" : st.picked.length === 1 ? "Tap the last, or use just this one" : "Range selected"}</span>
          {#each rangeActions as a (a.invoke.id)}<button type="button" class="p" data-range-action={a.invoke.id} disabled={!st.selectionRef} onclick={() => runAction(a.invoke, viewContext(a.invoke.id), { selection_ref: st.selectionRef })}>{a.invoke.title}</button>{/each}
          <button type="button" data-select-cancel onclick={() => st.endSelection()}>Cancel</button>{/if}
      </div>
    {/if}
    <div class="wrap">
      <div class="scroll" bind:this={el} {onscroll} tabindex="0" role="log" aria-live="polite" aria-label="Messages">
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
  .scroll { overflow-y: auto; flex: 1; min-height: 0; padding: var(--s-4) var(--s-5); display: flex; flex-direction: column; gap: var(--s-1); }
  .scroll > :global(.s) { gap: var(--s-1); }
  .msg { margin: auto; color: var(--ink-3); text-align: center; padding: var(--s-6); } .msg.bad { color: var(--danger); }
  .note { flex: none; margin: 0; padding: var(--s-2) var(--s-5); color: var(--warn); font-size: var(--text-label); } .note.ro { color: var(--ink-2); background: color-mix(in oklab, var(--ink) 6%, transparent); }
  .bar { flex: none; display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3); padding: var(--s-2) var(--s-5); }
  .bar button, .older, .chip { min-height: 36px; padding: 0 var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 9%, transparent); color: var(--ink); } .bar .p { background: var(--accent); color: var(--on-accent, #fff); } .bar button:disabled { opacity: 0.5; }
  .cnt { color: var(--ink-2); } .older { align-self: center; margin-bottom: var(--s-3); }
  .chip { position: absolute; top: var(--s-3); left: 50%; transform: translateX(-50%); box-shadow: 0 2px 10px color-mix(in oklab, #000 20%, transparent); }
  .jumps { position: absolute; right: var(--s-5); bottom: var(--s-4); display: grid; gap: var(--s-3); }
  .jb { position: relative; width: 44px; height: 44px; border-radius: 22px; background: var(--surface-solid, color-mix(in oklab, var(--ink) 14%, var(--paper, #fff))); color: var(--ink); font-size: 18px; box-shadow: 0 2px 10px color-mix(in oklab, #000 22%, transparent); }
  .jb .n { position: absolute; top: -6px; right: -6px; min-width: 20px; height: 20px; padding: 0 5px; box-sizing: border-box; border-radius: 10px; background: var(--accent); color: var(--on-accent, #fff); font-size: var(--text-label); display: grid; place-items: center; }
  .act { margin: var(--s-3) 0 0; color: var(--ink-3); font-style: italic; }
</style>
