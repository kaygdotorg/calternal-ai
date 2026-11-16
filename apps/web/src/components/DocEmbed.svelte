<script lang="ts">
  import * as api from '../lib/api';
  import { hrefFor } from '../lib/route';
  import type { Note } from '../lib/types';
  let { space, id }: { space: string; id: string } = $props();
  let note = $state<Note | null>(null);
  let err = $state<string | null>(null);
  $effect(() => {
    note = null; err = null;
    let live = true;
    api.invoke<{ note: Note }>('notes.get', { space_id: space, id }).then((d) => { if (live) note = d.note; }, (e) => { if (live) err = api.describeError(e); });
    return () => { live = false; };
  });
</script>

<div class="doc">
  {#if err}<div class="banner err">{err}</div>
  {:else if !note}<div class="skeleton"></div>
  {:else}
    <div class="h"><strong>{note.title}</strong> <a href={hrefFor({ name: 'notes', space, id })}>Open to edit</a></div>
    <textarea class="input" readonly rows="5" aria-label="Preview of {note.title} (read-only)" value={note.markdown}></textarea>
  {/if}
</div>

<style>
  .doc { display: grid; gap: 6px; } .h { display: flex; justify-content: space-between; gap: 10px; }
  textarea { font-family: var(--mono); font-size: 13px; resize: vertical; }
</style>
