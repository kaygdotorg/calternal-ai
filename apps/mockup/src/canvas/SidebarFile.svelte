<script lang="ts">
  // A sidebar opened as a collection: it IS a collection of collections, so its sections are its items, listed in order. They can be reordered here (drag the grip, or Alt+Up/Down on a row) and the same order is the sidebar's. "View file" shows the Markdown
  // file it is stored as (frontmatter, then an ordered list of links to its collections), written from the live arrangement: the proof that a sidebar is just a file beside the other collections.
  import { Sheet } from "@calternal/ui";
  import AppIcon from "../shell/AppIcon.svelte";
  import { arr } from "../arrange.svelte";
  let { sid, onopen }: { sid: string; onopen: (cid: string) => void } = $props();
  const name = $derived(arr.custom.find((c) => c.id === sid)?.name ?? `${sid[0]!.toUpperCase()}${sid.slice(1)}`);
  const secs = $derived(arr.sections(sid).filter((s) => arr.coll(s.cid)));
  const file = $derived(arr.sidebarFile(sid));
  let showFile = $state(false), from = $state<number | null>(null);
  function down(e: PointerEvent, i: number) {
    if (e.button !== 0) return; from = i; const list = (e.currentTarget as HTMLElement).closest(".sl")!; let at = i;
    const mv = (m: PointerEvent) => { const rows = [...list.querySelectorAll<HTMLElement>("[data-r]")]; let to = rows.findIndex((r) => m.clientY < r.getBoundingClientRect().top + r.getBoundingClientRect().height / 2); if (to < 0) to = rows.length - 1; if (to !== at) { arr.moveSection(sid, at, to); at = to; from = to; } };
    const up = () => { removeEventListener("pointermove", mv); removeEventListener("pointerup", up); from = null; };
    addEventListener("pointermove", mv); addEventListener("pointerup", up);
  }
  const key = (e: KeyboardEvent, i: number) => { if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return; e.preventDefault(); arr.moveSection(sid, i, i + (e.key === "ArrowUp" ? -1 : 1)); };
</script>

<div class="sf" data-sidebar-file>
  <header class="top"><p class="q">A sidebar is a collection of collections: {secs.length} {secs.length === 1 ? "section" : "sections"}, in this order.</p><button type="button" class="vf" onclick={() => (showFile = true)} data-view-file><AppIcon name="file" inline />View file</button></header>
  <ol class="sl">
    {#each secs as s, i (s.cid)}
      {@const c = arr.coll(s.cid)!}
      <li class="r" class:drag={from === i} data-r tabindex="-1">
        <span class="grip" role="presentation" aria-hidden="true" onpointerdown={(e) => down(e, i)}><AppIcon name="grip" inline /></span>
        <button type="button" class="b" onclick={() => onopen(s.cid)} onkeydown={(e) => key(e, i)} aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"><span class="cd" style:--hue={c.hue}></span><span class="nm">{s.name ?? c.name}</span><span class="m">{s.view ?? c.small} · {arr.ids(c.id).length} {arr.ids(c.id).length === 1 ? "item" : "items"}{s.hidden ? " · hidden" : ""}</span></button>
      </li>
    {/each}
    {#if !secs.length}<li class="em">No sections yet. Drag collections from the Collections tab into this sidebar.</li>{/if}
  </ol>
</div>
<Sheet open={showFile} label={`${file.path}`} side="center" onclose={() => (showFile = false)}>
  <div class="fv" data-file><p class="path">{file.path}</p><pre>{file.text}</pre></div>
</Sheet>

<style>
  .top { display: flex; align-items: center; justify-content: space-between; gap: var(--s-4); flex-wrap: wrap; padding-bottom: var(--s-4); } .q { margin: 0; color: var(--ink-3); font-size: var(--text-ui); } .vf { display: inline-flex; align-items: center; gap: var(--s-3); min-height: 28px; padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--hover); font-size: var(--text-label); }
  .sl { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-1); } .r { display: flex; align-items: center; border-radius: var(--r-select); transition: background-color var(--dur-fast) var(--ease-out); } .r:hover { background: var(--hover); } .r.drag { background: var(--accent-soft); }
  .grip { flex: none; display: grid; place-items: center; width: 28px; align-self: stretch; color: var(--ink-3); cursor: grab; touch-action: none; } .b { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h-settings); padding: 0 var(--s-4); text-align: start; font-size: var(--text-ui); }
  .cd { flex: none; width: 12px; height: 12px; border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); } .nm { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .m { color: var(--ink-3); font-size: var(--text-label); white-space: nowrap; } .em { color: var(--ink-3); padding: var(--s-5); }
  .fv { display: grid; gap: var(--s-3); padding: var(--s-6); width: min(36rem, 90vw); } .path { margin: 0; color: var(--ink-3); font-size: var(--text-label); font-family: var(--font-mono, monospace); } pre { margin: 0; padding: var(--s-5); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); font: var(--text-label) / 1.5 var(--font-mono, monospace); white-space: pre-wrap; overflow-x: auto; }
</style>
