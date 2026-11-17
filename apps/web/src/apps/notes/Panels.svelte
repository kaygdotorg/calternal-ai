<script lang="ts">
  // Notes right-card tabs (placeholder): the inspector and the style sheet.
  import { notes } from "./store.svelte";
  let { kind }: { id?: string; kind: "inspector" | "style" } = $props();
  // From the open note's text: words, and blocks (chunks of text between blank lines).
  const words = $derived(notes.note ? (notes.note.markdown.match(/\S+/g) ?? []).length : 0);
  const blocks = $derived(notes.note ? notes.note.markdown.split(/\n\s*\n/).filter((c) => c.trim()).length : 0);
</script>

<dl class="kv">
  {#if kind === "inspector"}
    <dt>Words</dt><dd>{words.toLocaleString()}</dd><dt>Blocks</dt><dd>{blocks.toLocaleString()}</dd><dt>Edited</dt><dd>{notes.note ? new Date(notes.note.updatedAt).toLocaleString("en-GB", { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }) : "-"}</dd><dt>Space</dt><dd>{notes.space?.name ?? "-"}</dd>
  {:else}
    <dt>Column</dt><dd>Narrow</dd><dt>Text</dt><dd>Google Sans Flex</dd><dt>Size</dt><dd>Body</dd>
  {/if}
</dl>

<style>
  .kv { display: grid; grid-template-columns: max-content 1fr; gap: var(--s-4) var(--s-7); margin: 0; padding: var(--s-4); font-size: var(--text-ui); }
  dt { color: var(--ink-3); } dd { margin: 0; }
</style>
