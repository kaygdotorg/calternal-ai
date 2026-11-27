<script lang="ts">
  // The page of a photo, an album or a library list opened in a window of its own (the seam's `itemPage` for the `photo` kind): the Photos page with that item selected, so an album shows its grid and a photo the viewer.
  import PhotosPage from "./PhotosPage.svelte";
  import type { Win } from "../../windows.svelte";
  import type { Item } from "../../data/collections";
  let { item }: { item: Item } = $props();
  const win = $state<Win>({ id: `pi-${Math.random().toString(36).slice(2, 7)}`, kind: "app", ref: "photos", sel: null, w: null });
  $effect.pre(() => { if (win.sel === null) win.sel = item.id; });
</script>

<PhotosPage {win} onopen={(i: Item) => (win.sel = i.id)} oncollection={() => {}} />
