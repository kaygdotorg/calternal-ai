<script lang="ts">
  // A note in a window that is not the focused one: the Notes app holds one open note (its store, its draft, its editor), so only the focused Notes window is the live editor. The others show a read-only copy,
  // read once and again when the window gets focus back. What the Notes app would need to have several editors open at once is in docs/ui-shell-status.md.
  import Md from "../lib/Md.svelte";
  import { notes } from "../apps/notes/store.svelte";
  let { item }: { item: string } = $props();
  let note = $state<{ title: string; markdown: string } | null>(null);
  let gone = $state(false);
  $effect(() => {
    const space = notes.space?.id;
    if (!space) return;
    let live = true;
    notes.client.get(space, item).then((n) => { if (live) note = n; }, () => { if (live) gone = true; });
    return () => { live = false; };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="np selectable" data-preview tabindex="0" role="region" aria-label="Note preview">
  {#if note}<Md text={note.markdown} />{:else if gone}<p class="q">That note does not exist, or you cannot see it.</p>{:else}<p class="q">Loading the note…</p>{/if}
  <p class="hint">A copy to read. Click the window to edit this note.</p>
</div>

<style>
  .np { height: 100%; overflow: auto; padding: calc(var(--pill-h) + var(--s-8)) var(--s-7) var(--s-6); font-size: var(--text-ui); }
  .q, .hint { color: var(--ink-3); font-size: var(--text-label); }
  .hint { margin-top: var(--s-6); }
</style>
