<script lang="ts">
  // A collection (`/collections/<id>`): a hand-picked set of notes, as multi-select makes it. Its title and notes (`collections.get`), each note a link, "Add notes" to put more in and a remove button on each to take one out
  // (`collections.set_members`, checked against the revision read: if someone changed it meanwhile it is read again and the person is told). Share opens the same sheet as a note's, for the collection. Who may change the notes
  // is the server's call; a refused change says so and changes nothing.
  import { IconButton, Pill, Row, SearchField, failed } from "@calternal/ui";
  import { say } from "../../server/say";
  import { ServerError } from "../../server/client";
  import { shell } from "../../shell/shell.svelte";
  import { href } from "../../lib/route.svelte";
  import { notes } from "./store.svelte";
  import { getCollection, setMembers, type Collection } from "./share/collections";
  import { incomingCollections } from "./share/incoming.svelte";
  import { openShare } from "./share/share.svelte";
  import ShareHost from "./share/ShareHost.svelte";
  import "./column.css";
  let { id }: { id?: string } = $props();
  let c = $state<Collection | null>(null), error = $state(""), busy = $state(false);
  let shareEl = $state<HTMLButtonElement>();
  async function read() {
    if (!id) return;
    try { c = await getCollection(id); error = ""; incomingCollections.read(id); }
    catch (e) { c = null; error = e instanceof ServerError && e.code === "not_found" ? "gone" : say(e, "open this collection"); }
  }
  $effect(() => { void id; c = null; error = ""; void read(); });
  /** Writes the new list of notes against the revision read; a stale one reads again and says why. */
  async function change(members: string[]) {
    if (!c || busy) return;
    busy = true;
    try { await setMembers(c.id, c.revision, members); }
    catch (e) { failed(e instanceof ServerError && e.code === "conflict" ? "Someone changed this collection at the same time. It is up to date again; try your change once more." : say(e, "change this collection")); }
    finally { busy = false; await read(); }
  }
  // Adding: find notes of the space by the start of their title, not already in it.
  let q = $state(""), hits = $state<{ id: string; title: string }[]>([]);
  $effect(() => {
    const text = q.trim(), space = notes.space?.id;
    if (!text || !space) { hits = []; return; }
    const h = setTimeout(() => void notes.client.lookupNotes(space, text).then((r) => (hits = r.filter((n) => !c?.members.some((m) => m.id === n.id)).slice(0, 6)), () => (hits = [])), 150);
    return () => clearTimeout(h);
  });
  const add = (noteId: string) => { q = ""; hits = []; void change([...(c?.members.map((m) => m.id) ?? []), noteId]); };
  const remove = (noteId: string) => void change((c?.members ?? []).filter((m) => m.id !== noteId).map((m) => m.id));
</script>

<div class="note">
  <div class="scroll">
    <header class="nc-head">
      {#if shell.narrow}<IconButton icon="sidebar" label="Show notes" onclick={() => shell.toggleSidebar()} />{/if}
      <span class="grow"></span>
      {#if c}<Pill icon="share" label="Share" aria-haspopup="dialog" bind:el={shareEl} onclick={() => openShare({ kind: "collections", id: c!.id }, c!.title, shareEl ?? null)} />{/if}
    </header>
    {#if c}<ShareHost item={{ kind: "collections", id: c.id }} />{/if}
    <article class="nc-article">
      {#if c}
        <h1>{c.title}</h1>
        <p class="lede">{c.members.length === 1 ? "1 note" : `${c.members.length} notes`}</p>
        {#if c.can_edit}<SearchField bind:value={q} label="Add notes" placeholder="Add notes" />{:else}<p class="lede">Only the space's owner can change what is in it.</p>{/if}
        {#if c.can_edit && hits.length}
          <ul class="rows" aria-label="Notes to add">{#each hits as n (n.id)}<li><Row title={n.title} icon="notes" onclick={() => add(n.id)} /></li>{/each}</ul>
        {:else if c.can_edit && q.trim()}<p class="lede">No note starts with “{q.trim()}”.</p>{/if}
        {#if c.members.length}
          <ul class="rows" aria-label="Notes in this collection">
            {#each c.members as m (m.id)}
              <li><Row title={m.title} icon="notes" href={href(`/notes/${m.id}`)}>{#snippet trailing()}{#if c!.can_edit}<IconButton small icon="close" label="Take {m.title} out of this collection" onclick={(e) => { e.preventDefault(); e.stopPropagation(); remove(m.id); }} />{/if}{/snippet}</Row></li>
            {/each}
          </ul>
        {:else}<p class="lede">{c.can_edit ? "Nothing in it yet. Add notes above." : "Nothing in it yet."}</p>{/if}
      {:else if error}
        <h1>{error === "gone" ? "Collection not found" : "Can't open this collection"}</h1>
        <p class="lede">{error === "gone" ? "It may have been removed, or you may not have access." : error}</p>
      {:else}<p class="lede" role="status">Opening…</p>{/if}
    </article>
  </div>
</div>

<style>
  .note { position: absolute; inset: 0; }
  .scroll { position: absolute; inset: 0; overflow-y: auto; }
  .grow { flex: 1; }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; letter-spacing: -0.01em; }
  .lede { color: var(--ink-3); font-size: var(--text-ui); }
  .rows { list-style: none; margin: var(--s-4) 0 0; padding: 0; display: grid; gap: var(--s-1); }
</style>
