<script lang="ts">
  // The sidebar of an app description: its title, sections (literal shortcut rows and/or a bound list each) in one scrolling column. Bound sections are windowed with `inside`, so the sidebar scrolls once.
  // A section without a title draws no header (and so no header actions). `showWhen` hides a section the route or role does not call for. Selectors and the footer follow in the next steps.
  import { Icon, Row } from "@calternal/ui";
  import type { Section, Sidebar } from "../model";
  import type { WindowStore } from "../windows/window-store.svelte";
  import ContentList from "./ContentList.svelte";
  import { knownIcon } from "./icons";
  import { marksOf } from "./eval";

  let { sidebar, storeFor, kindIconFor, selectedId, route, role, onopen, onsettle }: {
    sidebar: Sidebar; storeFor: (bindId: string) => WindowStore | undefined; kindIconFor: (bindId: string) => string | undefined;
    selectedId?: string | null; route: Record<string, string>; role?: "viewer" | "editor" | "manager" | "owner";
    onopen?: (bind: string, id: string, index: number) => void; onsettle?: (bind: string, id: string, index: number) => void;
  } = $props();

  const ranks = { viewer: 0, editor: 1, manager: 2, owner: 3 } as const;
  /** A section shows unless its `showWhen` says otherwise. */
  function shown(s: Section): boolean {
    const w = s.showWhen;
    if (!w) return true;
    if ("route" in w) return route[w.route] === w.equals;
    return (ranks[role ?? "viewer"] ?? 0) >= ranks[w.minRole];
  }
</script>

<nav class="sidebar" aria-label={sidebar.title ?? "Sidebar"} data-sidebar>
  {#if sidebar.title}<h2 class="title">{sidebar.title}</h2>{/if}
  <div class="scroll">
    {#each sidebar.sections ?? [] as s (s.id)}
      {#if shown(s)}
        <section class="sec" data-section={s.id}>
          {#if s.title}<h3 class="sh">{s.title}</h3>{/if}
          {#each s.rows ?? [] as r (r.id)}
            {@const icon = typeof r.icon === "string" ? knownIcon(r.icon) : undefined}
            <Row title={r.title} {icon}>
              {#snippet trailing()}{#each marksOf(r.marks, {}) as m, k (k)}<span data-mark={m.kind}>{m.text ?? ""}</span>{/each}{/snippet}
            </Row>
          {/each}
          {#if s.bind}
            {@const store = storeFor(s.bind)}
            {#if store}<ContentList {store} kindIcon={kindIconFor(s.bind)} label={s.title ?? s.id} inside {selectedId} onopen={(id, i) => onopen?.(s.bind!, id, i)} onsettle={(id, i) => onsettle?.(s.bind!, id, i)} />{/if}
          {/if}
        </section>
      {/if}
    {/each}
  </div>
</nav>

<style>
  .sidebar { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .title { margin: 0; padding: var(--s-5) var(--s-5) var(--s-3); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .scroll { flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding: 0 var(--s-3) var(--s-6); }
  .sh { margin: var(--s-5) 0 var(--s-2); padding: 0 var(--s-5); font-size: var(--text-section); font-weight: var(--strong-weight); color: var(--ink-3); text-transform: none; }
</style>
