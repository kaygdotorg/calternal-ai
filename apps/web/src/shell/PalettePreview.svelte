<script lang="ts">
  // The right column of the wide palette: what the highlighted row is, or what Enter will do. A note shows its title and text with the typed words marked; a thread its title and state; a command, a task, an ask or
  // a new item says what happens. The note is fetched once per row and kept (moving down the list never waits twice for the same note); the last one stays until the next arrives, so the pane never blinks empty.
  import { spaces } from "./spaces.svelte";
  import { store } from "../apps/threads/store.svelte";
  import type { Row } from "../palette/engine";
  let { row, words }: { row: Row | undefined; words: string } = $props();
  const cache = new Map<string, { title: string; text: string }>();
  let note = $state<{ title: string; text: string } | null>(null);
  const noteId = $derived(/^\/notes\/(.+)$/.exec(row?.action.go ?? "")?.[1] ?? null);
  const threadId = $derived(/^\/threads\/([^/]+)$/.exec(row?.action.go ?? "")?.[1] ?? null);
  const thread = $derived(threadId ? store.threads.find((t) => t.id === threadId) : undefined);
  $effect(() => {
    const id = noteId;
    if (!id) return;
    const hit = cache.get(id);
    if (hit) { note = hit; return; }
    let live = true;
    void import("../apps/notes/store.svelte").then(({ notes }) => notes.client.get(spaces.current.id, id)).then((n) => {
      const v = { title: n.title, text: n.markdown.replace(/^---\n[\s\S]*?\n---\n?/, "").trim().slice(0, 2400) };
      cache.set(id, v);
      if (live) note = v;
    }, () => { /* the row's own label stays */ });
    return () => { live = false; };
  });
  const terms = $derived(words.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter((t) => t.length > 1));
  /** The text as before and matched pieces, so a typed word shows as a mark. */
  function pieces(t: string): { s: string; hit: boolean }[] {
    if (!terms.length) return [{ s: t, hit: false }];
    const re = new RegExp(`(${terms.map((x) => x.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "giu");
    return t.split(re).filter(Boolean).map((s) => ({ s, hit: terms.includes(s.toLowerCase()) }));
  }
  const ENTER: Record<Row["kind"], string> = { open: "Opens it.", command: "Runs the command.", ask: "Sends the words to the agent. Nothing goes out until you press Enter.", task: "Makes a task from the words.", new: "Makes a new note from the words." };
</script>

<div class="pv" role="region" aria-label="Preview">
  {#if !row}
    <p class="soft">Type to search every space, go somewhere, run a command, or ask an agent.</p>
  {:else if noteId && note}
    <h2>{#each pieces(note.title) as g, i (i)}{#if g.hit}<mark>{g.s}</mark>{:else}{g.s}{/if}{/each}</h2>
    <p class="body">{#each pieces(note.text) as g, i (i)}{#if g.hit}<mark>{g.s}</mark>{:else}{g.s}{/if}{/each}</p>
  {:else}
    <h2>{row.label}</h2>
    {#if thread}<p class="soft">Thread · {thread.state === "idle" ? "no turn running" : thread.state}</p>{:else if row.sub}<p class="soft">{row.sub}</p>{/if}
    <p class="body">{ENTER[row.kind]}</p>
  {/if}
</div>

<style>
  .pv { height: 100%; overflow: auto; scrollbar-width: none; padding: var(--s-7) var(--s-8); display: grid; align-content: start; gap: var(--s-4); }
  h2 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); overflow-wrap: anywhere; }
  .body { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font-size: var(--text-ui); color: var(--ink-2); line-height: 1.5; }
  .soft { margin: 0; font-size: var(--text-label); color: var(--ink-3); }
  mark { color: inherit; background: color-mix(in oklab, var(--accent, currentColor) 26%, transparent); border-radius: var(--s-1); box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent, currentColor) 26%, transparent); }
</style>
