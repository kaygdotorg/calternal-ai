<script lang="ts">
  // The spike's host shell (host-drawn, not a plugin's): an app switcher, the app's sidebar, one window on the canvas with its
  // title bar, and the derived-form sheet. On a phone it is one column with Back. The address picks the opening state for
  // captures: ?app=<plugin>&open=<collection>&view=list|board&item=<file>&scheme=light|dark.
  import { IconButton, Selector } from "@calternal/ui";
  import { apps, collectionId, windowOf, type Json, type Row } from "./lib/model";
  import { draw, type Action } from "./lib/look";
  import { calls } from "./lib/calls.svelte";
  import Sidebar from "./views/Sidebar.svelte";
  import CollectionView from "./views/CollectionView.svelte";
  import ItemPage from "./views/ItemPage.svelte";
  import FormSheet from "./views/FormSheet.svelte";
  const q = new URLSearchParams(location.search);
  let appId = $state(q.get("app") ?? apps()[0].id);
  const app = $derived(apps().find((a) => a.id === appId)!);
  const findRow = (id: string | null): Row | null => { if (!id) return null; for (const c of app.sidebar) { const r = windowOf(collectionId(app, c)).rows.find((x) => x.item === id); if (r) return r; } return null; };
  let open = $state<{ c?: Json; row?: Row } | null>(null);
  let view = $state<"list" | "board">((q.get("view") as "board") ?? "list");
  $effect.pre(() => { void appId; const c = app.sidebar.find((s: Json) => s.collection === q.get("open")); const row = findRow(q.get("item")); open = row ? { row } : c ? { c } : null; });
  let sheet = $state<{ a: Action; prefill?: Record<string, string>; kind?: string } | null>(null);
  const act = (a: Action, prefill?: Record<string, string>) => (sheet = { a, prefill, kind: a.op.split("/")[0] + "/" + (app.operations?.[a.id]?.create ?? "") });
  const title = $derived(open?.row ? draw(open.row, "row").title : open?.c?.collection ?? app.name);
</script>

<div class="shell" class:has-open={!!open}>
  <aside class="side">
    <div class="top">
      <Selector label="Apps" size="sm" bind:value={appId} options={apps().map((a) => ({ value: a.id, label: a.name }))} onchange={() => (open = null)} />
    </div>
    <h1 class="wm">{app.name}</h1>
    <div class="scroll"><Sidebar {app} selected={open?.row?.item ?? null} onopen={(row) => (open = { row })} oncollection={(c) => (open = { c })} onaction={act} /></div>
    {#if calls.log.length}<div class="calls" aria-live="polite">{#each calls.log as l, i (i)}<code>{l.op} {JSON.stringify(l.inputs)}</code>{/each}</div>{/if}
  </aside>
  <main class="canvas">
    {#if open}
      <div class="win">
        <header class="bar"><span class="back"><IconButton icon="back" label="Back" small onclick={() => (open = null)} /></span><strong>{title}</strong><span class="sp"></span><span class="close"><IconButton icon="close" label="Close window" small onclick={() => (open = null)} /></span></header>
        <div class="body">
          {#if open.row}<ItemPage row={open.row} onaction={act} />
          {:else if open.c}<CollectionView {app} c={open.c} bind:view selected={null} onopen={(row) => (open = { row })} onaction={act} />{/if}
        </div>
      </div>
    {:else}<p class="hint">Open a collection or an item.</p>{/if}
  </main>
</div>
{#if sheet}<FormSheet action={sheet.a} kind={sheet.kind} prefill={sheet.prefill} onclose={() => (sheet = null)} />{/if}

<style>
  :global(body) { background: var(--bg, #ececee); } :global(html[data-scheme="dark"] body) { background: #161618; }
  .shell { display: grid; grid-template-columns: 300px 1fr; gap: var(--s-4); height: 100dvh; padding: var(--s-4); box-sizing: border-box; color: var(--ink); }
  .side { display: flex; flex-direction: column; min-height: 0; }
  .top { padding: var(--s-3) var(--s-4); } .wm { margin: var(--s-3) var(--s-5) 0; font-family: var(--font-brand, inherit); font-size: 1.6rem; font-weight: var(--strong-weight); }
  .scroll { flex: 1; overflow-y: auto; }
  .calls { display: grid; gap: 2px; padding: var(--s-3) var(--s-4); font-size: 11px; color: var(--ink-3); } .calls code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .canvas { min-width: 0; min-height: 0; display: flex; }
  .win { flex: 1; display: flex; flex-direction: column; min-height: 0; border-radius: var(--r-card, 18px); background: var(--card, #fff); box-shadow: 0 1px 3px rgb(0 0 0 / 0.08), 0 8px 30px rgb(0 0 0 / 0.06); overflow: hidden; }
  :global(html[data-scheme="dark"]) .win { --card: #232326; }
  .bar { flex: none; display: flex; align-items: center; gap: var(--s-3); padding: var(--s-3) var(--s-4); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.08); } .sp { flex: 1; } .back { display: none; }
  .body { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .hint { margin: auto; color: var(--ink-3); }
  @media (max-width: 719px) {
    .shell { grid-template-columns: 1fr; padding: 0; } .has-open .side, .shell:not(.has-open) .canvas { display: none; }
    .win { border-radius: 0; box-shadow: none; } .back { display: inline-flex; } .close { display: none; }
  }
</style>
