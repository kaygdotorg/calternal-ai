<script lang="ts">
  // The right card's three tabs for a thread. Placeholder content on fixture data: Changes lists what the turn changed (the real list
  // comes from the turn's recorded changes), Run shows where it runs, Files is the workspace tree. `kind` picks the tab.
  import { Badge, Icon, Row } from "@calternal/ui";
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
      <li><Row title={c.path.split("/").at(-1)!} sub={c.path} icon="file">
        {#snippet trailing()}<span class="add">+{c.added}</span><span class="rem">−{c.removed}</span><Badge text={c.status} tone={c.status === "added" ? "good" : "neutral"} />{/snippet}
      </Row></li>
    {/each}
  </ul>
{:else if kind === "run" && info}
  <div class="chips" aria-label="Access"><Badge text={info.permission} tone={info.permission === "Full access" ? "warn" : "neutral"} /><Badge text={info.runsOn} tone="info" /></div>
  <dl class="kv">
    {#each Object.entries({ State: info.state, Permission: info.permission, "Runs on": info.runsOn, Workspace: info.workspace, Branch: info.branch, Provider: info.provider, Model: info.model, Usage: info.tokens }) as [k, v]}<dt>{k}</dt><dd>{v}</dd>{/each}
  </dl>
{:else if kind === "files"}
  <ul class="list" aria-label="Workspace files">
    {#each TREE as [name, depth]}<li><Row title={name} icon={name.includes(".") ? "file" : "folder"} depth={depth - 1} /></li>{/each}
  </ul>
{/if}

<style>
  .chips { display: flex; flex-wrap: wrap; gap: var(--s-2); padding: var(--s-2) var(--s-4) 0; }
  .list { display: grid; gap: var(--s-1); }
  .empty { color: var(--ink-3); padding: var(--s-6); }
  .add { color: var(--good); font-variant-numeric: tabular-nums; } .rem { color: var(--danger); font-variant-numeric: tabular-nums; }
  .kv { display: grid; grid-template-columns: max-content 1fr; gap: var(--s-4) var(--s-7); margin: 0; padding: var(--s-4); font-size: var(--text-ui); }
  dt { color: var(--ink-3); } dd { margin: 0; overflow-wrap: anywhere; }
</style>
