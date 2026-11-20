<script lang="ts">
  // The snapshot nodes of a checked item view (docs/description-contract.md 5.1): stack, text, code, heading, image and attachment, drawn with the renderer's own tokens. Text and code are literal: nothing here
  // interprets markup, and no host chrome, confirmation or live action is drawn from a page. A node type this renderer does not know draws nothing (it is the host's to extend, not ours to guess at).
  import type { SnapshotNode } from "../draft";
  import Snapshot from "./Snapshot.svelte";
  let { node }: { node: SnapshotNode } = $props();
  /** The viewer's local time for a UTC instant, in the viewer's locale: the time today, the date and time otherwise. */
  function local(ts: string | undefined): string {
    if (!ts) return "";
    const d = new Date(ts); if (Number.isNaN(d.getTime())) return "";
    const same = new Date().toDateString() === d.toDateString();
    return new Intl.DateTimeFormat(undefined, same ? { timeStyle: "short" } : { dateStyle: "medium", timeStyle: "short" }).format(d);
  }
</script>

{#if node.type === "stack"}
  <div class="s" class:row={node.axis === "horizontal"} data-snapshot="stack">{#if node.title}<h3 class="st">{node.title}</h3>{/if}{#each node.children as c, k (k)}<Snapshot node={c} />{/each}</div>
{:else if node.type === "snapshot-heading"}
  <div class="h" data-snapshot="heading" data-anchor={node.anchor}><strong>{node.title}</strong>{#if node.author}<span class="a">{node.author}</span>{/if}{#if node.timestamp}<time datetime={node.timestamp}>{local(node.timestamp)}</time>{/if}</div>
{:else if node.type === "snapshot-text"}
  <p class="t {node.style ?? 'body'} {node.tone ?? ''}" data-snapshot="text" data-anchor={node.anchor}>{node.text}</p>
{:else if node.type === "snapshot-code"}
  <pre class="c" data-snapshot="code" data-anchor={node.anchor}><code>{node.text}</code></pre>
{:else if node.type === "snapshot-attachment"}
  <div class="att" data-snapshot="attachment" data-anchor={node.anchor} role="group" aria-label={node.label}>{node.label}</div>
{:else if node.type === "snapshot-image"}
  <div class="att" data-snapshot="image" data-anchor={node.anchor} role="img" aria-label={node.alt ?? "Image"}>{node.alt ?? "Image"}</div>
{/if}

<style>
  .s { display: flex; flex-direction: column; gap: var(--s-2); } .s.row { flex-direction: row; flex-wrap: wrap; gap: var(--s-3); }
  .st { margin: 0; font-size: var(--text-label); color: var(--ink-3); font-weight: var(--strong-weight); }
  .h { display: flex; flex-wrap: wrap; align-items: baseline; gap: var(--s-3); margin-top: var(--s-4); }
  .h .a { color: var(--ink-2); } .h time { color: var(--ink-3); font-size: var(--text-label); margin-inline-start: auto; }
  .t { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; } .t.title { font-size: var(--text-title, 28px); font-weight: var(--strong-weight); } .t.heading { font-weight: var(--strong-weight); } .t.caption { color: var(--ink-3); font-size: var(--text-label); }
  .t.good { color: var(--good); } .t.warn { color: var(--warn); } .t.bad { color: var(--danger); }
  .c { margin: 0; padding: var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 7%, transparent); overflow: auto; font-size: var(--text-label); font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace); }
  .att { padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 7%, transparent); width: fit-content; max-width: 100%; }
</style>
