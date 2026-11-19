<script lang="ts">
  // The Mail sidebar: accounts, folders and one folder's headers, drawn from a GenUI description built by the plugin's views (plugins/calternal-mail/src/views.ts). The description's buttons set
  // state; the store performs what that asks for and the description is drawn again. Remounted per description so a Set made before it does not outlive it.
  import { untrack } from "svelte";
  import type { Value } from "@calternal/genui";
  import GenUi from "../../genui/GenUi.svelte";
  import { effects } from "./effects";
  import { mail } from "./store.svelte";

  let values = $state<Record<string, Value>>({});
  let epoch = $state(0);
  const source = $derived(mail.listSource);
  $effect(() => {
    const v = values;
    untrack(() => {
      if (typeof v.go === "string" && v.go) epoch++;
      mail.apply(v, effects);
    });
  });
  $effect(() => { void mail.start(); });
</script>

<nav aria-label="Mail">
  <p class="section-heading first">Mail</p>
  <div class="body">{#key `${epoch}\0${source}`}<GenUi {source} bind:values />{/key}</div>
</nav>

<style>
  nav { display: grid; gap: var(--s-2); }
  .section-heading { margin-bottom: calc(var(--section-below) - var(--s-2)); }
  /* The description sits in the sidebar's own gutter (the heading's), and nothing in it may be wider than the sidebar: grids take the column's width, a long subject is cut with an ellipsis instead of widening the list. */
  .body { padding: 0 var(--s-5); min-width: 0; }
  .body :global(.gen), .body :global(.stack) { grid-template-columns: minmax(0, 1fr); }
  .body :global(.rowl) { flex-wrap: nowrap; min-width: 0; }
  .body :global(.rowl > button) { justify-content: flex-start; text-align: start; min-width: 0; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .body :global(.rowl > button > *) { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .body :global(select) { min-width: 0; }
</style>
