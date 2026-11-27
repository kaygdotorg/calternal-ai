<script lang="ts">
  // The page of a file or folder opened in a window of its own (the seam's `itemPage` for the `file` kind): the Files page with that item selected, so a folder shows its contents and a file its preview.
  import FilesPage from "./FilesPage.svelte";
  import type { Win } from "../../windows.svelte";
  import type { Item } from "../../data/collections";
  let { item }: { item: Item } = $props();
  const win = $state<Win>({ id: `fi-${Math.random().toString(36).slice(2, 7)}`, kind: "app", ref: "files", sel: null, w: null });
  $effect.pre(() => { if (win.sel === null) win.sel = item.id; });
</script>

<FilesPage {win} onopen={(i: Item) => (win.sel = i.id)} oncollection={() => {}} />
