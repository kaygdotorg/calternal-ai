<script lang="ts">
  // Draws a generated-UI description (packages/genui) with the shell's own components. The description is compiled once per source
  // (cached), evaluated again only when a state value changes, and walked by <GenNode>. A node the catalog has but this client does
  // not draw yet shows a plain fallback line; an evaluation Error node shows its message. Nothing here runs code from the description.
  import { compile, evaluate, formatDiagnostics, type Program, type Value } from "@calternal/genui";
  import { perform, type Action, type Host } from "./actions";
  import GenNode from "./GenNode.svelte";

  let { source, onsend, values = $bindable({}) }: { source: string; onsend?: (text: string) => void; values?: Record<string, Value> } = $props();

  const cache = new Map<string, { program: Program | null; error: string }>();
  function compiled(src: string) {
    let c = cache.get(src);
    if (!c) {
      const r = compile(src);
      c = { program: r.program, error: r.program ? "" : formatDiagnostics(r.diagnostics) };
      if (cache.size > 200) cache.clear();
      cache.set(src, c);
    }
    return c;
  }
  const c = $derived(compiled(source));
  let override = $state<Record<string, Value> | undefined>();
  const snap = $derived(c.program ? evaluate(c.program, override) : null);
  $effect(() => { if (snap) values = snap.state; });
  const host: Host = {
    set: (name, value) => { override = { ...(snap?.state ?? {}), [name]: value }; },
    get send() { return onsend; },
  };
  const act = (a: Action[] | undefined) => { for (const x of a ?? []) perform(x, host); };
</script>

{#if c.program && snap}
  <div class="gen">
    {#each snap.roots as n (n.key)}<GenNode node={n} {host} {act} />{/each}
  </div>
{:else}
  <p class="err" role="alert">This card could not be drawn: {c.error}</p>
{/if}

<style>
  .gen { display: grid; gap: var(--s-5); min-width: 0; }
  .err { color: var(--danger); font-size: var(--text-label); }
</style>
