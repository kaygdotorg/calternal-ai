<script lang="ts">
  // One row of a right-panel list (Changes, Files, Run), the same anatomy in all three: the icon sits on the first line (centred on the title, not on the
  // whole block); the title, then the secondary line (a path, or a label) a line gap below; trailing columns on the right (counts, status) that keep their
  // width, so the figures of different rows line up. Spacing is all from the scale: s-4 inside, s-3 between rows, s-5 between the icon and the words.
  import { Icon, type IconName } from "@calternal/ui";
  import type { Snippet } from "svelte";
  let { icon, title, sub, depth = 0, trailing }: { icon: IconName; title: string; sub?: string; depth?: number; trailing?: Snippet } = $props();
</script>

<div class="pr" style:--depth={depth}>
  <span class="ic"><Icon name={icon} /></span>
  <span class="tx"><span class="t">{title}</span>{#if sub}<span class="s">{sub}</span>{/if}</span>
  {#if trailing}<span class="tr">{@render trailing()}</span>{/if}
</div>

<style>
  .pr { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; column-gap: var(--s-4); align-items: start; padding: var(--s-4) var(--s-5) var(--s-4) calc(var(--s-5) + var(--depth) * var(--s-7)); border-radius: var(--r-select); }
  /* The icon's box is one line tall, so it centres on the title's first line. */
  .ic { display: grid; place-items: center; height: calc(var(--text-ui) * 1.45); color: var(--ink-2); }
  .tx { min-width: 0; display: grid; row-gap: var(--s-1); }
  .t { font-size: var(--text-ui); line-height: 1.45; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .s { font-size: var(--text-label); line-height: 1.4; color: var(--ink-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tr { display: inline-flex; align-items: center; gap: var(--s-3); height: calc(var(--text-ui) * 1.45); font-size: var(--text-label); }
  @media (hover: hover) and (pointer: fine) { .pr:hover { background: var(--hover); } }
</style>
