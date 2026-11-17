<script lang="ts">
  // Notes placeholder: proves the middle card's second layout. A narrow centred column with no inner card, and a thin rail at the card's
  // left edge that expands into a small table-of-contents card (hover, focus or press; Escape folds it). Fixture text only.
  import { Panel } from "@calternal/ui";
  import { shell } from "../../shell/shell.svelte";
  import { IconButton } from "@calternal/ui";
  import { InlineEditor, Button } from "@calternal/ui";
  import Md from "../../lib/Md.svelte";
  import { notes } from "./store.svelte";
  let { id }: { id?: string } = $props();
  $effect(() => { void notes.open(id); });
  const sections = $derived((notes.note?.markdown.match(/^## .+$/gm) ?? []).map((h) => h.slice(3)));
  let editing = $state(false);
  let title = $state("");
  let body = $state("");
  const edit = () => { title = notes.note?.title ?? ""; body = notes.note?.markdown ?? ""; editing = true; };
  async function done() { editing = false; if (notes.note && (title !== notes.note.title || body !== notes.note.markdown)) await notes.save(title, body); }
  let toc = $state(false);
  const uid = $props.id();
</script>

<div class="note">
  <div class="rail" role="presentation" onpointerenter={() => (toc = true)} onpointerleave={() => (toc = false)}>
    <button type="button" class="ticks" aria-expanded={toc} aria-controls="{uid}-toc" aria-label="Table of contents" onclick={() => (toc = !toc)} onfocus={() => (toc = true)} onkeydown={(e) => e.key === "Escape" && (toc = false)}>
      {#each sections as _}<i></i>{/each}
    </button>
    {#if toc}
      <Panel class="tocard" radius="card" dense floating id="{uid}-toc" role="navigation" aria-label="Contents">
        {#each sections as s, i}<a href="#s{i}" onclick={(e) => e.preventDefault()}>{s}</a>{/each}
      </Panel>
    {/if}
  </div>

  <div class="scroll">
  <header>
    {#if shell.narrow || !shell.sidebarOpen}<IconButton icon="sidebar" label="Show notes" onclick={() => shell.toggleSidebar()} />{/if}
    <span class="grow"></span>
    {#if !shell.narrow}<IconButton icon="sidebar-right" label={shell.rightOpen ? "Hide the side card" : "Show the side card"} pressed={shell.rightOpen} onclick={() => shell.toggleRight()} />{/if}
  </header>

  <article>
    {#if notes.note}
      {#if editing}
        <InlineEditor class="t-edit" label="Title" bind:value={title} />
        <InlineEditor class="b-edit" multiline maxRows={30} label="Note" bind:value={body} />
        <div><Button variant="primary" text="Done" onclick={done} /></div>
      {:else}
        <h1 ondblclick={edit}>{notes.note.title}</h1>
        <Md text={notes.note.markdown.replace(/^# .+\n*/, "")} />
        <div><Button variant="quiet" text="Edit" onclick={edit} /></div>
      {/if}
      {#if notes.error}<p class="err" role="alert">{notes.error}</p>{/if}
    {:else}
      <h1>{id ? "Note not found" : "Notes"}</h1>
      <p class="lede">{notes.error || (id ? "It may have been removed, or you may not have access." : "Choose a note on the left, or make a new one.")}</p>
    {/if}
  </article>
  </div>
</div>

<style>
  .note { position: absolute; inset: 0; }
  .scroll { position: absolute; inset: 0; overflow-y: auto; }
  header { position: sticky; top: 0; z-index: 2; display: flex; align-items: center; gap: var(--s-4); min-height: calc(var(--bar-h) + var(--safe-top)); padding: var(--safe-top) var(--s-5) 0; }
  .grow { flex: 1; }
  article { max-width: 40rem; margin: 0 auto; padding: var(--s-5) var(--pad-region) calc(var(--s-10) + var(--bottom-inset, 0px)); display: grid; gap: var(--s-5); }
  .err { color: var(--danger); font-size: var(--text-label); }
  :global(.t-edit) { font-size: var(--text-title); font-weight: var(--strong-weight); width: 100%; }
  :global(.b-edit) { width: 100%; min-height: 12rem; line-height: 1.55; }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; letter-spacing: -0.01em; }
  p { font-size: var(--text-ui); line-height: 1.65; font-weight: var(--body-weight); }
  .lede { color: var(--ink-3); }
  .rail { position: absolute; z-index: 3; top: 50%; translate: 0 -50%; left: 0; }
  .ticks { display: grid; gap: var(--s-4); padding: var(--s-6) var(--s-5); min-width: var(--pill-h); justify-items: start; }
  .ticks i { display: block; width: var(--s-6); height: 2px; border-radius: var(--r-pill); background: var(--ink-4); }
  .ticks i:first-child { width: var(--s-8); background: var(--ink-2); }
  :global(.tocard) { position: absolute; left: var(--s-5); top: 50%; translate: 0 -50%; display: grid; min-width: 15rem; padding: var(--s-3); --origin: left center; }
  :global(.tocard) a { padding: var(--s-3) var(--s-5); border-radius: calc(var(--r-card) - var(--s-3)); font-size: var(--text-ui); color: var(--ink); }
  @media (hover: hover) { :global(.tocard) a:hover { background: var(--hover); } }
</style>
