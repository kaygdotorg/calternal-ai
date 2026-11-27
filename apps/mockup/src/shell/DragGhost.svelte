<script lang="ts">
  // What follows the pointer while a collection is dragged: its name on a small glass pill, with a note of what releasing does (add here, move here, or remove when a section is over nothing).
  import { Panel } from "@calternal/ui";
  import { dnd } from "../dnd.svelte";
  import { ITEMS } from "../data/collections";
  import { arr } from "../arrange.svelte";
  const d = $derived(dnd.drag);
  const name = $derived(!d ? "" : d.kind === "row" ? (ITEMS[d.row!]?.title ?? "") : (arr.coll(d.cid)?.name ?? ""));
  const hint = $derived(!d ? "" : d.kind === "row" ? (d.over ? (arr.coll(d.target ?? "")?.pin && d.target !== d.cid ? "Pin here" : "Move here") : arr.coll(d.cid)?.pin ? "Release to unpin" : "Drop on a section") : d.over ? (d.kind === "coll" ? "Add as a section" : "Move here") : d.kind === "section" ? "Release to remove" : "Drop on the sidebar");
</script>

{#if d}
  <div class="g" style:left="{d.x + 14}px" style:top="{d.y + 10}px" class:rm={!d.over && (d.kind === "section" || (d.kind === "row" && !!arr.coll(d.cid)?.pin))} data-ghost aria-hidden="true">
    <Panel radius="pill"><span class="in"><strong>{name}</strong><span>{hint}</span></span></Panel>
  </div>
{/if}

<style>
  .g { position: fixed; z-index: 90; pointer-events: none; translate: 0 0; } .in { display: grid; padding: var(--s-2) var(--s-5); font-size: var(--text-ui); } .in span { color: var(--ink-3); font-size: var(--text-label); } .rm .in span { color: var(--bad, #c0392b); }
</style>
