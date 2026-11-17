<script lang="ts">
  // The Settings page: an empty shell for now. The title bar and the section's one-line purpose show; the pages themselves are not built (docs/settings.md
  // decides what each holds; the layout waits for the study of Apple's settings windows, issue #22). Appearance opens the sheet that already exists (the address /settings/appearance).
  import { sectionById } from "./sections";
  let { id }: { id?: string } = $props();
  const section = $derived(sectionById(id));
</script>

<div class="page">
  <h1>{section?.name ?? "Settings"}</h1>
  {#if section}
    <p>{section.blurb}</p>
    <p class="soft">{section.id === "appearance" ? "Appearance opens as a sheet until its page is designed." : "This page is not built yet."}</p>
  {:else}
    <p class="soft">Choose a section on the left.</p>
  {/if}
</div>

<style>
  .page { display: grid; align-content: start; justify-items: start; gap: var(--s-5); max-width: 40rem; margin: 0 auto; padding: calc(var(--bar-h) + var(--s-6)) var(--s-7) var(--s-8); }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); }
  .soft { color: var(--ink-3); }
</style>
