<script lang="ts">
  // The space switcher: a row of dots, one per space, in the middle of the sidebar's footer (like the profile dots of the Aside browser). The current space is the larger, filled dot; hovering,
  // focusing or long-pressing a dot says the space's name (the tooltip); a click or tap switches. Choosing a space changes what the sidebar lists, and takes the middle card to the app's start so it
  // never shows an item the sidebar does not list.
  // Motion, from the tokens: press scales the dot at once (`--press-scale`); the marker moves from one dot to the next as size and opacity over `--dur-base` on `--ease-out`; the list under the dots
  // fades in over the same (Sidebar.svelte). Interruptible: only CSS transitions, so a second tap simply redirects them.
  import { tip } from "@calternal/ui";
  import { navigate, segments } from "../lib/route.svelte";
  import { spaces } from "./spaces.svelte";
  function pick(id: string) {
    if (id === spaces.current.id) return;
    spaces.switchTo(id);
    const kind = segments()[0];
    navigate(kind === "notes" || kind === "threads" ? `/${kind}` : "/threads");
  }
</script>

{#if spaces.list.length > 1}
  <div class="dots" role="radiogroup" aria-label="Spaces">
    {#each spaces.list as s (s.id)}
      <button type="button" role="radio" class="dot" aria-checked={s.id === spaces.current.id} aria-label={s.name} use:tip={{ label: s.name }} onclick={() => pick(s.id)}><i></i></button>
    {/each}
  </div>
{/if}

<style>
  .dots { display: flex; align-items: center; justify-content: center; gap: 0; }
  /* The touch target is wider than the dot. */
  .dot { width: var(--s-8); height: var(--pill-h); display: grid; place-items: center; border-radius: var(--r-pill); }
  .dot i { display: block; width: var(--s-3); height: var(--s-3); border-radius: 50%; background: var(--ink-3); opacity: 0.55; transition: width var(--dur-base) var(--ease-out), height var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out), background-color var(--dur-base) var(--ease-out); }
  .dot:hover i { opacity: 0.9; }
  .dot[aria-checked="true"] i { width: var(--s-4); height: var(--s-4); opacity: 1; background: var(--accent); }
  .dot:active i { scale: var(--press-scale); }
</style>
