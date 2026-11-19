<script lang="ts">
  // One item row, drawn from the bind's row template: a leading icon where the template declares one (an icon the set lacks keeps its place as a neutral mark), the title on one line, up to three marks.
  // Everything shown comes from the template and the row's fields; nothing is added or dropped here. Selection is read from the list, never from the description.
  import { Icon, Row } from "@calternal/ui";
  import type { RowFields, RowTemplate, WindowRow } from "../model";
  import { iconOf, marksOf, titleOf } from "./eval";
  import { knownIcon } from "./icons";
  let { template, row, kindIcon, selected = false, depth = 0, onclick }: { template: RowTemplate; row: WindowRow; kindIcon?: string; selected?: boolean; depth?: number; onclick?: (e: MouseEvent) => void } = $props();
  const fields = $derived(row.fields as RowFields);
  const title = $derived(titleOf(template, fields));
  const iconName = $derived(iconOf(template.icon, fields, kindIcon));
  const icon = $derived(knownIcon(iconName));
  const marks = $derived(marksOf(template.marks, fields));
</script>

<Row {title} {selected} {depth} {onclick}>
  {#snippet leading()}{#if iconName !== undefined}<span class="lead">{#if icon}<Icon name={icon} />{:else}<i class="missing" data-icon-missing={iconName}></i>{/if}</span>{/if}{/snippet}
  {#snippet trailing()}{#each marks as m, k (k)}
    {#if m.kind === "dot"}<i class="dot {m.tone ?? ''}" data-mark="dot"></i>
    {:else if m.kind === "icon"}<span class="mi {m.tone ?? ''}" data-mark="icon">{#if knownIcon(m.icon)}<Icon name={knownIcon(m.icon)!} inline />{:else}<i class="missing small" data-icon-missing={m.icon}></i>{/if}</span>
    {:else}<span class="mt {m.tone ?? ''}" data-mark={m.kind}>{m.text}</span>{/if}
  {/each}{/snippet}
</Row>

<style>
  .lead { flex: none; display: grid; color: var(--ink-2); }
  .missing { width: var(--glyph); height: var(--glyph); border-radius: 50%; border: 1.5px dashed currentColor; opacity: 0.4; box-sizing: border-box; }
  .missing.small { width: var(--glyph-inline); height: var(--glyph-inline); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }
  .dot.good, .mi.good, .mt.good { --c: var(--good); } .dot.warn, .mi.warn, .mt.warn { --c: var(--warn); } .dot.bad, .mi.bad, .mt.bad { --c: var(--danger); }
  .dot.good, .dot.warn, .dot.bad { background: var(--c); }
  .mi, .mt { color: var(--c, var(--ink-3)); }
  .mt { font-variant-numeric: tabular-nums; }
</style>
