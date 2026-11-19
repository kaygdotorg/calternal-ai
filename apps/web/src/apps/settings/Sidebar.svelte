<script lang="ts">
  // Settings sidebar: the sections in the groups of docs/ui-craft-and-settings-study.md 11.1 (User, Admin: this space, Admin: server), one selected row.
  import { listNav, Row } from "@calternal/ui";
  import { href, segments } from "../../lib/route.svelte";
  import { visibleSections } from "./registry";
  const current = $derived(segments()[0] === "settings" ? segments()[1] : undefined);
</script>

<nav aria-label="Settings" use:listNav={{ rows: "a.row" }}>
  {#each visibleSections() as g (g.id)}
    <div class="grp" role="group" aria-labelledby="g-{g.id}">
      <h2 class="section-heading" id="g-{g.id}">{g.name}</h2>
      <ul>{#each g.pages as p (p.id)}<li><Row title={p.name} icon={p.icon} href={href(`/settings/${p.id}`)} selected={current === p.id} /></li>{/each}</ul>
    </div>
  {/each}
</nav>

<style>
  nav { display: grid; padding-bottom: var(--section-above); }
  .grp:first-child h2 { margin-top: 0; }
  ul { display: grid; gap: var(--s-1); }
</style>
