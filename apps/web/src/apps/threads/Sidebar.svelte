<script lang="ts">
  // The Threads app's sidebar body: the space's threads grouped by workspace, children nested under their parent, with running and
  // waiting states. A state is an icon-free dot plus a word for assistive tech; colour alone never carries it.
  import { Row } from "@calternal/ui";
  import { href, route, segments } from "../../lib/route.svelte";
  import type { ThreadState, ThreadSummary } from "./model";
  import { store } from "./store.svelte";
  const current = $derived(segments()[0] === "threads" ? segments()[1] : undefined);
  void route;
  const time = (t: number) => new Date(t).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" });
  const WORD: Record<ThreadState, string> = { idle: "", running: "running", waiting: "waiting for you", failed: "failed", done: "finished" };
</script>

{#snippet line(t: ThreadSummary, depth: number)}
  <Row title={t.title} sub={WORD[t.state] ? `${WORD[t.state]} · ${time(t.updatedAt)}` : time(t.updatedAt)} href={href(`/threads/${t.id}`)} selected={current === t.id} {depth}>
    {#snippet leading()}<span class="dot {t.state}" role="img" aria-label={WORD[t.state] || "idle"}></span>{/snippet}
  </Row>
{/snippet}

<nav aria-label="Threads">
  {#each store.groups as g (g.workspace)}
    <h2>{g.workspace}</h2>
    <ul>
      {#each g.threads as t (t.id)}
        <li>{@render line(t, 0)}</li>
        {#each store.childrenOf(t.id) as c (c.id)}<li>{@render line(c, 1)}</li>{/each}
      {/each}
    </ul>
  {/each}
</nav>

<style>
  nav { display: grid; gap: var(--s-1); padding-bottom: var(--s-6); }
  h2 { padding: var(--s-5) var(--s-4) var(--s-2); font-size: var(--text-section); font-weight: var(--strong-weight); letter-spacing: 0.05em; text-transform: uppercase; color: var(--ink-3); }
  ul { display: grid; gap: var(--s-1); }
  .dot { flex: none; width: var(--s-4); height: var(--s-4); border-radius: 50%; background: var(--ink-4); opacity: 0.5; }
  .dot.running { background: var(--accent); opacity: 1; animation: pulse 1.6s var(--ease-out) infinite; }
  .dot.waiting { background: var(--warn); opacity: 1; }
  .dot.failed { background: var(--danger); opacity: 1; }
  .dot.done { background: var(--good); opacity: 0.8; }
  @keyframes pulse { 50% { scale: 0.7; opacity: 0.55; } }
  @media (prefers-reduced-motion: reduce) { .dot.running { animation: none; } }
</style>
