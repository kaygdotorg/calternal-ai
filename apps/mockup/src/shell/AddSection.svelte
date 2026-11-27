<script lang="ts">
  // "Add a section": the + at the end of a sidebar opens the search surface (the same GlassSheet as search and New space) to pick a collection already in the space, or to create a new one by typing its name. A section is a collection shown small; the pick puts it at the end of the sidebar. Fixture only.
  import { SearchField, toast } from "@calternal/ui";
  import GlassSheet from "./GlassSheet.svelte";
  import { arr } from "../arrange.svelte";
  import { LIBRARY } from "../data/collections";
  let { open, sid, onclose }: { open: boolean; sid: string; onclose: () => void } = $props();
  let q = $state("");
  const here = $derived(new Set(arr.sections(sid).map((s) => s.cid)));
  const all = $derived([...LIBRARY, ...Object.keys(arr.extraIds())].filter((id, i, a) => a.indexOf(id) === i && arr.coll(id)));
  const found = $derived(all.filter((id) => arr.coll(id)!.name.toLowerCase().includes(q.trim().toLowerCase())));
  const exact = $derived(all.some((id) => arr.coll(id)!.name.toLowerCase() === q.trim().toLowerCase()));
  const done = () => { q = ""; onclose(); };
  function add(cid: string) { if (here.has(cid)) { toast(`${arr.coll(cid)!.name} is already in this sidebar`); return; } arr.insertSection(sid, arr.sections(sid).length, cid); toast(`Added ${arr.coll(cid)!.name}`); done(); }
  function create() { const n = q.trim(); if (!n) return; arr.addSection(sid, n); toast(`Created ${n}`); done(); }
</script>

<GlassSheet {open} label="Add a section" onclose={done}>
  <div class="as" data-add-section>
    <SearchField bind:value={q} label="Find or create a collection" placeholder="Find a collection, or type a name to create one" />
    <ul aria-label="Collections">
      {#each found as id (id)}<li><button type="button" onclick={() => add(id)}><i style:--hue={arr.coll(id)!.hue}></i><span>{arr.coll(id)!.name}</span><small>{here.has(id) ? "In this sidebar" : `${arr.items(id).length} items`}</small></button></li>{/each}
      {#if q.trim() && !exact}<li><button type="button" class="mk" onclick={create}><b>+</b><span>Create a collection named “{q.trim()}”</span></button></li>{/if}
      {#if !found.length && !q.trim()}<li class="n">No collections yet.</li>{/if}
    </ul>
  </div>
</GlassSheet>

<style>
  .as { display: grid; gap: var(--s-4); padding: var(--s-5); min-width: min(34rem, 88vw); max-height: 70dvh; } ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 1px; overflow: auto; }
  li button { width: 100%; display: flex; align-items: center; gap: var(--s-5); min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); text-align: start; color: var(--ink); font-size: var(--text-ui); } li button:hover { background: var(--hover); } li i { width: 12px; height: 12px; border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); flex: none; } li span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } small { color: var(--ink-3); font-size: var(--text-label); } .mk b { width: 12px; text-align: center; } .n { padding: var(--s-5); color: var(--ink-3); }
</style>
