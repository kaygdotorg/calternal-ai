<script lang="ts">
  // The note: the block editor in a narrow centred column with no inner card, and a thin rail at the card's left edge that expands into a small contents card (hover, focus or press;
  // Escape folds it). The document is the note's Markdown (the fixture's or the server's); every edit is kept as the note's private draft after a short pause and saved after a longer one.
  import { Button, IconButton, Panel } from "@calternal/ui";
  import { shell } from "../../shell/shell.svelte";
  import NoteEditor from "./NoteEditor.svelte";
  import { notes } from "./store.svelte";
  import { store as threadStore } from "../threads/store.svelte";
  import { navigate } from "../../lib/route.svelte";
  // Tags in use: fixture words until the server lists them (its search index will).
  const TAGS = ["design", "journal", "release", "storage", "reading"];
  let { id }: { id?: string } = $props();
  $effect(() => { void notes.open(id); });
  let heads = $state<{ level: number; text: string; from: number }[]>([]);
  let editor = $state<NoteEditor>();
  let latest = "";
  let draftTimer = 0, saveTimer = 0;
  const titleOf = (t: string) => /^# +(.+)$/m.exec(t)?.[1]?.trim() ?? notes.note?.title ?? "Untitled";
  function edited(text: string) {
    latest = text;
    clearTimeout(draftTimer); clearTimeout(saveTimer);
    draftTimer = window.setTimeout(() => void notes.keepDraft(text), 600);
    saveTimer = window.setTimeout(() => void flush(), 1500);
  }
  async function flush() {
    clearTimeout(saveTimer);
    if (notes.note && latest && latest !== notes.note.markdown) await notes.save(titleOf(latest), latest);
  }
  $effect(() => () => { if (latest) void flush(); });
  let toc = $state(false);
  const uid = $props.id();
</script>

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
  <header>
    {#if shell.narrow}<IconButton icon="sidebar" label="Show notes" onclick={() => shell.toggleSidebar()} />{/if}
    <span class="grow"></span>
  </header>

  <article>
    {#if notes.note}
      {#if notes.draft}
        <p class="draft" role="status">You have an unsent draft of this note. <Button variant="quiet" text="Restore" onclick={() => { editor?.setText(notes.draft!.text); edited(notes.draft!.text); }} /> <Button variant="quiet" text="Discard" onclick={async () => { await notes.input.dropDraft(notes.note!.spaceId, notes.note!.id); await notes.reloadInput(); }} /></p>
      {/if}
      <NoteEditor bind:this={editor} id={notes.note.id} text={notes.note.markdown} onedit={edited} onheadings={(h) => (heads = h)} label={notes.note.title} notes={notes.list} threads={threadStore.threads.map((t) => ({ id: t.id, title: t.title }))} agents={threadStore.providers.map((p) => p.name)} tags={TAGS} />
      {#if notes.error}<p class="err" role="alert">{notes.error}</p>{/if}
    {:else}
      <h1>{id ? "Note not found" : "Notes"}</h1>
      <p class="lede">{notes.error || (id ? "It may have been removed, or you may not have access." : notes.list.length ? "Choose a note on the left, or make a new one." : "A note is a page of Markdown that you and your agents can read and edit. This space has none yet.")}</p>
      {#if !id && !notes.list.length && !notes.error}<p class="first"><Button variant="primary" text="Write the first note" onclick={async () => { const n = await notes.create("Untitled"); if (n) navigate(`/notes/${n.id}`); }} /></p>{/if}
    {/if}
  </article>
  </div>
</div>

<style>
  .note { position: absolute; inset: 0; }
  .scroll { position: absolute; inset: 0; overflow-y: auto; }
  header { position: sticky; top: 0; z-index: 2; display: flex; align-items: center; gap: var(--s-4); min-height: calc(var(--bar-h) + var(--safe-top)); padding: var(--safe-top) var(--s-5) 0 calc(var(--s-5) + var(--edge-l, 0px)); }
  .grow { flex: 1; }
  article { max-width: 40rem; margin: 0 auto; padding: var(--s-5) var(--pad-region) calc(var(--s-10) + var(--bottom-inset, 0px)); display: grid; gap: var(--s-5); }
  .draft { padding: var(--s-3) var(--s-5); border-radius: var(--r-card); background: var(--accent-soft); font-size: var(--text-label); display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-2); }
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
