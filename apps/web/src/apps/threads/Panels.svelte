<script lang="ts">
  // The right card's three tabs for a thread. Placeholder content on fixture data: Changes lists what the turn changed (the real list
  // comes from the turn's recorded changes), Run shows where it runs, Files is the workspace tree. `kind` picks the tab.
  import { Badge } from "@calternal/ui";
  import PanelRow from "./PanelRow.svelte";
  import type { Change, RunInfo } from "./model";
  import { client } from "./store.svelte";
  let { id, kind }: { id?: string; kind: "changes" | "run" | "files" } = $props();
  let changes = $state<Change[]>([]);
  let info = $state<RunInfo>();
  $effect(() => {
    if (!id) return;
    void client.changes(id).then((c) => (changes = c));
    void client.run(id).then((r) => (info = r));
  });
  const TREE = [["crates", 1], ["store", 2], ["journal.rs", 3], ["receipts.rs", 3], ["index.rs", 3], ["docs", 1], ["storage-layout.md", 2], ["scripts", 1], ["check-docs.sh", 2]] as const;
</script>

{#if !id}
  <p class="empty">Open a thread to see its {kind}.</p>
{:else if kind === "changes"}
  <ul class="list" aria-label="Changed files">
    {#each changes as c (c.path)}
      <li><PanelRow title={c.path.split("/").at(-1)!} sub={c.path} icon="file">
        {#snippet trailing()}<span class="n add">+{c.added}</span><span class="n rem">−{c.removed}</span><span class="st"><Badge text={c.status} tone={c.status === "added" ? "good" : "neutral"} /></span>{/snippet}
      </PanelRow></li>
    {/each}
  </ul>
{:else if kind === "run" && info}
  <ul class="list" aria-label="Run details">
    {#each [["State", info.state, "pulse"], ["Permission", info.permission, "shield"], ["Runs on", info.runsOn, "terminal"], ["Workspace", info.workspace, "folder"], ["Branch", info.branch, "branch"], ["Provider", info.provider, "spark"], ["Model", info.model, "code"], ["Usage", info.tokens, "list"]] as const as [k, v, ic]}
      <li><PanelRow icon={ic} title={v} sub={k} /></li>
    {/each}
  </ul>
{:else if kind === "files"}
  <ul class="list" aria-label="Workspace files">
    {#each TREE as [name, depth]}<li><PanelRow title={name} icon={name.includes(".") ? "file" : "folder"} depth={depth - 1} /></li>{/each}
  </ul>
{/if}

<style>
  .list { display: grid; gap: var(--s-3); }
  .empty { color: var(--ink-3); padding: var(--s-6); }
  /* Trailing columns keep their widths so the figures and badges of different rows line up. */
  .n { min-width: 4.4ch; text-align: end; font-variant-numeric: tabular-nums; }
  .add { color: var(--good); } .rem { color: var(--danger); }
  .st { min-width: 6.2em; display: inline-flex; justify-content: flex-end; }
</style>
