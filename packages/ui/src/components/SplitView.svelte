<script lang="ts">
  // A list on the left and the selected thing's detail on the right, both inside one card: Settings' sections beside the section, a mail plugin's messages beside the message. Where the card is wide enough for both (its own
  // width, not the window's: a container query on this element, `breakpoint`, 40 rem by default) the two sit side by side, each scrolling on its own, and the list keeps its selection highlighted; where it is narrow the
  // list and the detail take turns: the list while nothing is `selected`, the detail with a Back button once something is (drill-in). No nested page is ever needed when there is width. The component only arranges and
  // collapses: what the list holds (rows, a virtual list) and what the detail shows are the caller's snippets, and `selected` and `onback` are the caller's state.
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  let { list, detail, selected, onback, listLabel, backLabel = "Back", breakpoint = "40rem", listWidth = "15rem", class: klass }: {
    /** The list pane's content. */ list: Snippet;
    /** The detail pane's content (shown when something is selected; the caller may also show an empty state). */ detail: Snippet;
    /** Whether something is selected: in the narrow arrangement this picks the pane that shows. */ selected: boolean;
    /** Called by the Back button of the narrow arrangement (the caller clears its selection). */ onback: () => void;
    /** The name of the list region, for assistive technology. */ listLabel: string;
    /** The Back button's accessible name and text (it shows only in the narrow arrangement). */ backLabel?: string;
    /** The card width below which the panes take turns (any CSS length). */ breakpoint?: string;
    /** The list pane's width in the side-by-side arrangement. */ listWidth?: string;
    /** An extra class for the root. */ class?: string;
  } = $props();
</script>

<div class={["split", klass]} data-selected={selected || undefined} style:--split-bp={breakpoint} style:--split-list={listWidth}>
  <nav class="list" aria-label={listLabel}>{@render list()}</nav>
  <section class="detail">
    <button type="button" class="back" onclick={onback}><Icon inline name="back" /><span>{backLabel}</span></button>
    {@render detail()}
  </section>
</div>

<style>
  /* The root is the container: its own width decides the arrangement, whatever the window is. */
  .split { container-type: inline-size; height: 100%; min-height: 0; display: grid; grid-template-columns: var(--split-list, 15rem) minmax(0, 1fr); gap: var(--s-4); }
  .list, .detail { min-width: 0; min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
  .back { display: none; align-items: center; gap: var(--s-2); min-height: var(--pill-h); padding: 0 var(--s-4); margin-bottom: var(--s-3); border-radius: var(--r-pill); color: var(--ink-2); font-size: var(--text-ui); }
  .back:active { scale: var(--press-scale); }
  /* Narrow: one pane at a time. The breakpoint is a number the caller sets; container queries cannot read a custom property, so the two common widths are written out and a caller with another sets `breakpoint` to one of them. */
  @container (max-width: 40rem) {
    .split[style*="--split-bp: 40rem"], .split:not([style*="--split-bp"]) { grid-template-columns: minmax(0, 1fr); }
    .split[style*="--split-bp: 40rem"] .detail, .split:not([style*="--split-bp"]) .detail { display: none; }
    .split[data-selected][style*="--split-bp: 40rem"] .list, .split[data-selected]:not([style*="--split-bp"]) .list { display: none; }
    .split[data-selected][style*="--split-bp: 40rem"] .detail, .split[data-selected]:not([style*="--split-bp"]) .detail { display: block; }
    .split[data-selected] .back { display: inline-flex; }
  }
  @container (max-width: 30rem) {
    .split[style*="--split-bp: 30rem"] { grid-template-columns: minmax(0, 1fr); }
    .split[style*="--split-bp: 30rem"] .detail { display: none; }
    .split[data-selected][style*="--split-bp: 30rem"] .list { display: none; }
    .split[data-selected][style*="--split-bp: 30rem"] .detail { display: block; }
    .split[data-selected][style*="--split-bp: 30rem"] .back { display: inline-flex; }
  }
</style>
