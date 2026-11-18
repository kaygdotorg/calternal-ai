<script lang="ts">
  // A list on the left and the selected thing's detail on the right, both inside one card, each scrolling on its own: Settings' sections beside the section. It is only the side-by-side
  // arrangement. Whether there is room for it is the caller's decision (it knows its card's width): a narrow card shows its content as one flat page instead, so there is never a list-then-detail
  // drill-in and never a Back button. The list keeps its selection highlighted; what each pane holds is the caller's snippet.
  import type { Snippet } from "svelte";
  let { list, detail, listLabel, detailLabel = "Details", listWidth = "15rem", class: klass }: {
    /** The list pane's content. */ list: Snippet;
    /** The detail pane's content. */ detail: Snippet;
    /** The name of the list region, for assistive technology. */ listLabel: string;
    /** The name of the detail region, for assistive technology. */ detailLabel?: string;
    /** The list pane's width. */ listWidth?: string;
    /** An extra class for the root. */ class?: string;
  } = $props();
</script>

<div class={["split", klass]} style:--split-list={listWidth}>
  <nav class="list" aria-label={listLabel}>{@render list()}</nav>
  <section class="detail" aria-label={detailLabel}>{@render detail()}</section>
</div>

<style>
  .split { height: 100%; min-height: 0; display: grid; grid-template-columns: var(--split-list, 15rem) minmax(0, 1fr); gap: var(--s-4); }
  .list, .detail { min-width: 0; min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
</style>
