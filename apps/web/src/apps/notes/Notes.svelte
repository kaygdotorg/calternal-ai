<script lang="ts">
  // The note: the block editor in a narrow centred column with no inner card, and a thin rail at the card's left edge that expands into a small contents card (hover, focus or press;
  // Escape folds it). The document is the note's Markdown (the fixture's or the server's); every edit is kept as the note's private draft after a short pause and saved after a longer one.
  import { getContext, untrack } from "svelte";
  import { Avatar, Button, IconButton, Menu, Panel, Pill, tip } from "@calternal/ui";
  import ShareHost from "./share/ShareHost.svelte";
  import { openShare } from "./share/share.svelte";
  import { incoming } from "./share/incoming.svelte";
  import { liveNotice, liveOf, liveOn, settleOwedNow } from "./live/live.svelte";
  import Presence from "./live/Presence.svelte";
  import { shell } from "../../shell/shell.svelte";
  import NoteEditor from "./NoteEditor.svelte";
  import TrashView from "./TrashView.svelte";
  import { notes } from "./store.svelte";
  import "./column.css";
  import "./style/page.css";
  import { noteStyle } from "./style/store.svelte";
  import { hasSurface, pageTheme, pageVars } from "./style/apply";
  import { store as threadStore } from "../threads/store.svelte";
  import { navigate, route } from "../../lib/route.svelte";
  import { serverMode } from "../../server/client";
  import { link } from "../../server/link.svelte";
  // Tags in use: fixture words until the server lists them (its search index will).
  // Fixture words for the # picker; in server mode the editor asks the space's tag index instead.
  const TAGS = ["design", "journal", "release", "storage", "reading"];
  let { id }: { id?: string } = $props();
  // Live editing (`?live=1`): the note is a room, other people's cursors and presence show, and the room (not an HTTP save) keeps the text. Off by default; nothing of it loads when off.
  const liveMode = liveOn();
  // Edits owed to the personal space after access ended are sent now and whenever the browser comes back online.
  $effect(() => {
    const go = () => void settleOwedNow();
    go(); addEventListener("online", go);
    return () => removeEventListener("online", go);
  });
  const noteId = $derived(notes.note?.id);
  /** This note's room: a window in the canvas has its own, so several notes can be live at once. */
  const live = $derived(liveOf(noteId ?? ""));
  $effect(() => {
    const nid = noteId;
    if (!liveMode || !nid) return;
    const room = liveOf(nid);
    untrack(() => void room.open(notes.note!.spaceId, nid, notes.note!.markdown));
    return () => room.close();
  });
  let moreOpen = $state(false);
  let moreEl = $state<HTMLButtonElement>();
  let shareEl = $state<HTMLButtonElement>();
  /** How the open note reached the person, when someone else owns it: their avatar then sits where Share is, and opens the same sheet. */
  const sharedBy = $derived(incoming.of(notes.note?.id));
  /** In a canvas window the window's own pill (three buttons) floats at the top right: the save state leaves room for it and the sidebar's way back is the shell's. */
  const windowed = getContext<boolean | undefined>("canvas-window") === true;
  // Leaving a note saves what is pending first (against that note), then the next one opens.
  $effect(() => { void notes.open(id); incoming.read(id ?? ""); return () => { clearTimeout(saveTimer); if (latest) void flush(); latest = ""; }; });
  // The editor was given new text (a note opened, changed elsewhere, or the other version taken): what was typed before no longer applies.
  $effect(() => { void notes.view.seq; latest = ""; });
  let heads = $state<{ level: number; text: string; from: number }[]>([]);
  let editor = $state<NoteEditor>();
  const style = $derived(noteStyle.page);
  const surface = $derived(hasSurface(style, noteStyle.fm));
  let latest = "";
  let draftTimer = 0, saveTimer = 0;
  const titleOf = (t: string) => /^# +(.+)$/m.exec(t)?.[1]?.trim() ?? notes.note?.title ?? "Untitled";
  function edited(text: string) {
    latest = text;
    notes.touch();
    clearTimeout(draftTimer); clearTimeout(saveTimer);
    draftTimer = window.setTimeout(() => void notes.keepDraft(text), 600);
    saveTimer = window.setTimeout(() => void flush(), 1500);
  }
  // One save at a time; text typed while it runs is saved by the next one (the editor is never touched by a save).
  let inflight = false, again = false;
  async function flush() {
    clearTimeout(saveTimer);
    if (!notes.note || !latest || latest === notes.note.markdown || notes.saveState === "conflict") return;
    if (inflight) { again = true; return; }
    inflight = true;
    try { await notes.save(titleOf(latest), latest); } finally { inflight = false; }
    if (again) { again = false; void flush(); }
  }
  $effect(() => () => { if (latest) void flush(); });
  const STATE = { saved: "Saved", dirty: "Not saved yet", saving: "Saving…", conflict: "Changed elsewhere", waiting: "Can't reach the server. Your text is still here and saves when it is back", failed: "Not saved" } as const;
  // The server is back: send what could not be sent.
  $effect(() => { if (link.state === "ok" && notes.saveState === "waiting") void flush(); });
  // While the server is away a save is not failing, it is waiting (the client repeats it with its original request id until the server answers): the line says the text is safe, as it does for a write that was queued.
  const stateLine = $derived(link.state === "reconnecting" && (notes.saveState === "saving" || notes.saveState === "dirty") ? STATE.waiting : STATE[notes.saveState]);
  let toc = $state(false);
  const uid = $props.id();
</script>

{#if route.query.get("view") === "trash"}
  <TrashView />
{:else}
<div class="note">
  <div class="rail" role="presentation" onpointerenter={() => (toc = true)} onpointerleave={() => (toc = false)}>
    <button type="button" class="ticks" aria-expanded={toc} aria-controls="{uid}-toc" aria-label="Table of contents" onclick={() => (toc = !toc)} onfocus={() => (toc = true)} onkeydown={(e) => e.key === "Escape" && (toc = false)}>
      {#each heads.filter((h) => h.level > 1) as _}<i></i>{/each}
    </button>
    {#if toc}
      <Panel class="tocard" radius="card" dense floating id="{uid}-toc" role="navigation" aria-label="Contents">
        {#each heads as h}<a href="#{h.from}" class="l{h.level}" onclick={(e) => { e.preventDefault(); editor?.scrollTo(h.from); toc = false; }}>{h.text}</a>{:else}<span class="empty">No headings yet</span>{/each}
      </Panel>
    {/if}
  </div>

  <div class="scroll">
  <header class="nc-head" class:windowed>
    {#if shell.narrow && !windowed}<IconButton icon="sidebar" label="Show notes" onclick={() => shell.toggleSidebar()} />{/if}
    <span class="grow"></span>
    {#if liveMode && live.session && !windowed}<Presence here={live.here} />{/if}
    {#if notes.note}<span class="state" role="status" aria-live="polite" data-state={liveMode ? undefined : notes.saveState}>{liveMode ? live.save : stateLine}</span>{/if}
    {#if !windowed && notes.note}
      {#if sharedBy}
        <button type="button" class="owner" bind:this={shareEl} aria-label="Shared by {sharedBy.shared_by}. Who can see this" aria-haspopup="dialog" use:tip={{ label: `Shared by ${sharedBy.shared_by}`, side: "bottom" }} onclick={() => openShare({ kind: "notes", id: notes.note!.id }, notes.note!.title, shareEl ?? null)}><Avatar name={sharedBy.shared_by} size="md" /></button>
      {:else}
        <Pill icon="share" label="Share" aria-haspopup="dialog" bind:el={shareEl} onclick={() => openShare({ kind: "notes", id: notes.note!.id }, notes.note!.title, shareEl ?? null)} />
      {/if}
    {/if}
    <!-- The side panel's only button is an item of this menu (a window in the canvas has the same item in its own three-dot menu). -->
    {#if !windowed}
      <Pill icon="more" label="Note actions" aria-haspopup="menu" aria-expanded={moreOpen} bind:el={moreEl} onclick={() => (moreOpen = !moreOpen)} />
      <Menu open={moreOpen} items={[{ id: "panel", label: shell.rightOpen ? "Hide side panel" : "Show side panel", icon: "sidebar-right" }]} label="Note actions" trigger={moreEl} morph onclose={() => (moreOpen = false)} onpick={() => { moreOpen = false; shell.toggleRight(); }} />
    {/if}
  </header>

  {#if notes.note}<ShareHost item={{ kind: "notes", id: notes.note.id }} />{/if}

  <article class="nc-article" use:pageTheme={style.theme} data-width={style.width === "narrow" ? undefined : style.width} data-size={style.size === "default" ? undefined : style.size} data-font={style.font === "system" ? undefined : style.font}
    data-surface={surface || undefined} data-frame={surface && style.frame ? "" : undefined} data-grain={surface && style.grain ? "" : undefined} data-faint={style.opacity !== null && style.opacity < 0.6 ? "" : undefined} style={pageVars(style)}>
    {#if notes.note}
      {#if notes.draft}
        <p class="draft" role="status">You have an unsent draft of this note. <Button variant="quiet" text="Restore" onclick={() => { editor?.setText(notes.draft!.text); edited(notes.draft!.text); }} /> <Button variant="quiet" text="Discard" onclick={async () => { await notes.input.dropDraft(notes.note!.spaceId, notes.note!.id); await notes.reloadInput(); }} /></p>
      {/if}
      {#snippet editorEl(room?: { extension: import("@codemirror/state").Extension }, text?: string)}
        <NoteEditor bind:this={editor} id={notes.view.id} text={text ?? notes.view.text} live={room} onedit={edited} onheadings={(h) => (heads = h)} label={notes.note!.title} readOnly={sharedBy?.permission === "view"} notes={notes.list} threads={threadStore.threads.map((t) => ({ id: t.id, title: t.title }))} agents={threadStore.providers.map((p) => p.name)} tags={TAGS} />
      {/snippet}
      {#if !liveMode}{@render editorEl()}
      {:else if live.binding && live.session}{#key live.epoch}{@render editorEl(live.binding, live.session.text.toString())}{/key}
      {:else}<p class="lede" role="status">Opening the note…</p>{/if}
      {#if notes.conflict}
        <p class="draft" role="alert">Someone changed this note while you were editing it. Your text is kept here. <Button variant="primary" text="Keep mine" onclick={() => notes.keepMine(titleOf(editor?.current() ?? latest), editor?.current() ?? latest)} /> <Button variant="quiet" text="Use theirs" onclick={() => notes.takeTheirs()} /></p>
      {/if}
      {#if notes.error}<p class="err" role="alert">{notes.error}</p>{/if}
      {#if liveNotice.text}<p class="err" role="alert">{liveNotice.text}</p>{/if}
    {:else}
      <h1>{notes.error ? (notes.errorCode === "unavailable" ? "Can't reach the server" : notes.errorCode === "forbidden" ? "You can't open this note" : id ? "Note not found" : "Notes") : id ? "Note not found" : "Notes"}</h1>
      <p class="lede">{notes.error || (id ? "It may have been removed, or you may not have access." : notes.list.length ? "Choose a note on the left, or make a new one." : "A note is a page of Markdown that you and your agents can read and edit. This space has none yet.")}</p>
      {#if notes.error}<p class="first"><Button variant="primary" text="Try again" onclick={() => notes.retry(id)} /></p>{/if}
      {#if !id && !notes.list.length && !notes.error}<p class="first"><Button variant="primary" text="Write the first note" onclick={async () => { const n = await notes.create("Untitled"); if (n) navigate(`/notes/${n.id}`); }} /></p>{/if}
    {/if}
  </article>
  </div>
</div>
{/if}

<style>
  .note { position: absolute; inset: 0; }
  .scroll { position: absolute; inset: 0; overflow-y: auto; }
  .grow { flex: 1; }
  .owner { flex: none; display: inline-grid; border-radius: 50%; transition: scale var(--dur-fast) var(--ease-out); }
  .owner:active { scale: var(--press-scale); }
  .draft { padding: var(--s-3) var(--s-5); border-radius: var(--r-card); background: var(--accent-soft); font-size: var(--text-label); display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-2); }
  .state { font-size: var(--text-label); color: var(--ink-3); }
  .nc-head.windowed { padding-right: calc(var(--pill-h) * 3 + var(--s-8)); }
  .state[data-state="conflict"], .state[data-state="failed"] { color: var(--danger); }
  .err { color: var(--danger); font-size: var(--text-label); }
  :global(.tocard) a.l2 { padding-left: var(--s-6); } :global(.tocard) a.l3 { padding-left: var(--s-8); }
  .empty { padding: var(--s-3) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; letter-spacing: -0.01em; }
  p { font-size: var(--text-ui); line-height: 1.65; font-weight: var(--body-weight); }
  .lede { color: var(--ink-3); }
  .first { margin-top: var(--s-5); }
  .rail { position: absolute; z-index: 3; top: 50%; translate: 0 -50%; left: 0; }
  .ticks { display: grid; gap: var(--s-4); padding: var(--s-6) var(--s-5); min-width: var(--pill-h); justify-items: start; }
  .ticks i { display: block; width: var(--s-6); height: 2px; border-radius: var(--r-pill); background: var(--ink-4); }
  .ticks i:first-child { width: var(--s-8); background: var(--ink-2); }
  :global(.tocard) { position: absolute; left: var(--s-5); top: 50%; translate: 0 -50%; display: grid; min-width: 15rem; padding: var(--s-3); --origin: left center; }
  :global(.tocard) a { padding: var(--s-3) var(--s-5); border-radius: calc(var(--r-card) - var(--s-3)); font-size: var(--text-ui); color: var(--ink); }
  @media (hover: hover) { :global(.tocard) a:hover { background: var(--hover); } }
</style>
