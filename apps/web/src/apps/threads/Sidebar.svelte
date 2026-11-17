<script lang="ts">
  // The Threads app's sidebar body: the space's threads grouped by workspace, children nested under their parent. A thread's state shows only when it
  // means something (running, waiting for you, failed), as an icon and a word on its quiet line; idle and finished threads show just the time. A thread
  // that is in the attention card above is not listed here (its children, if any, stand on their own).
  import { Row } from "@calternal/ui";
  import { href, segments } from "../../lib/route.svelte";
  import StateMark from "./StateMark.svelte";
  import type { ThreadSummary } from "./model";
  import { store } from "./store.svelte";
  const current = $derived(segments()[0] === "threads" ? segments()[1] : undefined);
  const time = (t: number) => new Date(t).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" });
  const hidden = $derived(new Set(store.attention.slice(0, 3).map((a) => a.thread.id)));
  const shown = $derived(store.threads.filter((t) => !hidden.has(t.id)));
  const groups = $derived.by(() => {
    const ids = new Set(shown.map((t) => t.id));
    const by = new Map<string, ThreadSummary[]>();
    for (const t of shown) if (!t.parentId || !ids.has(t.parentId)) (by.get(t.workspace) ?? by.set(t.workspace, []).get(t.workspace)!).push(t);
    return [...by].map(([workspace, threads]) => ({ workspace, threads: threads.sort((a, b) => b.updatedAt - a.updatedAt) }));
  });
  const kids = (id: string) => shown.filter((t) => t.parentId === id).sort((a, b) => b.updatedAt - a.updatedAt);
</script>

{#snippet line(t: ThreadSummary, depth: number)}
  <Row title={t.title} href={href(`/threads/${t.id}`)} selected={current === t.id} {depth}>
    {#snippet meta()}{#if t.state === "running" || t.state === "waiting" || t.state === "failed"}<StateMark state={t.state} /><span aria-hidden="true">·</span>{/if}<span>{time(t.updatedAt)}</span>{/snippet}
  </Row>
{/snippet}

<nav aria-label="Threads">
  {#each groups as g (g.workspace)}
    <h2>{g.workspace}</h2>
    <ul>
      {#each g.threads as t (t.id)}
        <li>{@render line(t, 0)}</li>
        {#each kids(t.id) as c (c.id)}<li>{@render line(c, 1)}</li>{/each}
      {/each}
    </ul>
  {/each}
</nav>

<style>
  nav { display: grid; gap: var(--s-1); padding-bottom: var(--s-6); }
  h2 { padding: var(--s-5) var(--s-4) var(--s-2); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  ul { display: grid; gap: var(--s-1); }
</style>
