<script lang="ts">
  // Notes right-card tabs: the inspector (counts) and the Style sheet (components in ./style; the bottom bar's Style entry hosts the same sheet).
  import { notes } from "./store.svelte";
  import StyleSheet from "./style/StyleSheet.svelte";
  let { kind }: { id?: string; kind: "inspector" | "style" } = $props();
  // From the open note's text: words, and blocks (chunks of text between blank lines).
  const words = $derived(notes.note ? (notes.note.markdown.match(/\S+/g) ?? []).length : 0);
  const blocks = $derived(notes.note ? notes.note.markdown.split(/\n\s*\n/).filter((c) => c.trim()).length : 0);
</script>

{#if kind === "style"}
  <StyleSheet />
{:else}
  <dl class="kv">
    <dt>Words</dt><dd>{words.toLocaleString()}</dd><dt>Blocks</dt><dd>{blocks.toLocaleString()}</dd><dt>Edited</dt><dd>{notes.note ? new Date(notes.note.updatedAt).toLocaleString("en-GB", { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }) : "-"}</dd><dt>Space</dt><dd>{notes.space?.name ?? "-"}</dd>
  </dl>
{/if}

<style>
  .kv { display: grid; grid-template-columns: max-content 1fr; gap: var(--s-4) var(--s-7); margin: 0; padding: var(--s-4); font-size: var(--text-ui); }
  dt { color: var(--ink-3); } dd { margin: 0; }
</style>
