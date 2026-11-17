<script lang="ts">
  // Notes placeholder sidebar: a flat list of fixture notes.
  import { Row } from "@calternal/ui";
  import { segments } from "../../lib/route.svelte";
  const NOTES = [["n-1", "Journal replay notes"], ["n-2", "Storage layout"], ["n-3", "Invite flow"], ["n-4", "Release checklist"], ["n-5", "Reading list"]] as const;
  const current = $derived(segments()[0] === "notes" ? segments()[1] : undefined);
</script>

<nav aria-label="Notes"><ul>{#each NOTES as [id, t]}<li><Row title={t} icon="notes" href="/notes/{id}" selected={current === id} /></li>{/each}</ul></nav>

<style>
  ul { display: grid; gap: var(--s-1); }
</style>
