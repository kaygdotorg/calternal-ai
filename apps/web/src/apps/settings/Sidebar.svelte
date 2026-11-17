<script lang="ts">
  // Settings sidebar: the sections grouped User and Admin, like every other app's sidebar (links, one selected).
  import { Row } from "@calternal/ui";
  import { href, segments } from "../../lib/route.svelte";
  import { GROUPS } from "./sections";
  const current = $derived(segments()[0] === "settings" ? segments()[1] : undefined);
</script>

<nav aria-label="Settings">
  {#each GROUPS as g (g.id)}
    <section aria-labelledby="g-{g.id}">
      <h2 id="g-{g.id}">{g.name}</h2>
      <ul>{#each g.sections as s (s.id)}<li><Row title={s.name} icon={s.icon} href={href(`/settings/${s.id}`)} selected={current === s.id} /></li>{/each}</ul>
    </section>
  {/each}
</nav>

<style>
  nav { display: grid; gap: var(--s-5); }
  h2 { padding: 0 var(--s-5); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  ul { display: grid; gap: var(--s-1); }
</style>
