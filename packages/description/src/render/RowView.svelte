<script lang="ts">
  // One item row, drawn from the bind's row template: a leading icon where the template declares one (an icon the set lacks keeps its place as a neutral mark), the title on one line, up to three marks.
  // Everything shown comes from the template and the row's fields; nothing is added or dropped here. Selection is read from the list, never from the description.
  import { Icon, Row } from "@calternal/ui";
  import type { RowFields, RowTemplate, WindowRow } from "../model";
  import { iconOf, imageIconOf, marksOf, secondaryOf, secondaryParts, titleOf } from "./eval";
  import RowImage from "./RowImage.svelte";
  import Avatar from "./Avatar.svelte";
  import { knownIcon } from "./icons";
  let { template, row, kindIcon, selected = false, depth = 0, onclick }: { template: RowTemplate; row: WindowRow; kindIcon?: string; selected?: boolean; depth?: number; onclick?: (e: MouseEvent) => void } = $props();
  const fields = $derived(row.fields as RowFields);
  const title = $derived(titleOf(template, fields));
  const iconName = $derived(iconOf(template.icon, fields, kindIcon));
  const icon = $derived(knownIcon(iconName));
  const img = $derived(imageIconOf(template.icon));
  const avatar = $derived(template.icon && typeof template.icon === "object" && "avatar" in template.icon ? (template.icon as unknown as { avatar: { label: string; image?: string } }).avatar : null);
  const sub = $derived(secondaryOf(template.secondary, fields));
  const parts = $derived(secondaryParts(template.secondary, fields));
  const marks = $derived(marksOf(template.marks, fields));
</script>

{#snippet metaLine()}{#if parts.length > 1}<span class="sl">{parts.slice(0, -1).join(" \u00b7 ")}</span><span class="sx">\u00b7 {parts.at(-1)}</span>{:else}<span class="sl">{parts[0] ?? ""}</span>{/if}{/snippet}
<Row {title} {sub} {selected} {depth} {onclick} meta={parts.length ? metaLine : undefined}>
  {#snippet leading()}{#if avatar}<Avatar label={String(fields[avatar.label] ?? title ?? "")} id={row.id} image={avatar.image && typeof fields[avatar.image] === "string" ? (fields[avatar.image] as string) : undefined} />{:else if img}<RowImage ref={typeof fields[img.field] === "string" ? (fields[img.field] as string) : undefined} snippet={img.fallback && typeof fields[img.fallback] === "string" ? (fields[img.fallback] as string) : undefined} />{:else if iconName !== undefined}<span class="lead">{#if icon}<Icon name={icon} />{:else}<i class="missing" data-icon-missing={iconName}></i>{/if}</span>{/if}{/snippet}
  {#snippet trailing()}{#each marks as m, k (k)}
    {#if m.kind === "dot"}<i class="dot {m.tone ?? ''}" data-mark="dot"></i>
    {:else if m.kind === "icon"}<span class="mi {m.tone ?? ''}" data-mark="icon">{#if knownIcon(m.icon)}<Icon name={knownIcon(m.icon)!} inline />{:else}<i class="missing small" data-icon-missing={m.icon}></i>{/if}</span>
    {:else if m.kind === "count"}<span class="cnt {m.tone ?? ''}" data-mark="count">{m.text}</span>
    {:else}<span class="mt {m.tone ?? ''}" data-mark={m.kind}>{m.text}</span>{/if}
  {/each}{/snippet}
</Row>

<style>
  .sl { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .sx { flex: none; white-space: nowrap; }
  .lead { flex: none; display: grid; color: var(--ink-2); }
  .missing { width: var(--glyph); height: var(--glyph); border-radius: 50%; border: 1.5px dashed currentColor; opacity: 0.4; box-sizing: border-box; }
  .missing.small { width: var(--glyph-inline); height: var(--glyph-inline); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }
  .dot.good, .mi.good, .mt.good { --c: var(--good); } .dot.warn, .mi.warn, .mt.warn { --c: var(--warn); } .dot.bad, .mi.bad, .mt.bad { --c: var(--danger); }
  .dot.good, .dot.warn, .dot.bad { background: var(--c); }
  .mi, .mt { color: var(--c, var(--ink-3)); }
  .mt { font-variant-numeric: tabular-nums; white-space: nowrap; max-width: var(--time-max, 56px); overflow: hidden; text-overflow: ellipsis; }
  /* A count is a pill: the accent by default (the unread mark), the secondary fill for `quiet` (a muted chat), tone colours as marks elsewhere. */
  .cnt { min-width: 20px; height: 20px; padding: 0 6px; box-sizing: border-box; border-radius: 10px; display: inline-grid; place-items: center; font-size: var(--text-label); font-weight: var(--strong-weight); font-variant-numeric: tabular-nums; background: var(--c, var(--accent)); color: var(--on-accent, #fff); }
  .cnt.quiet { background: color-mix(in oklab, var(--ink) 16%, transparent); color: var(--ink-2); }
  .cnt.good { --c: var(--good); } .cnt.warn { --c: var(--warn); } .cnt.bad { --c: var(--danger); }
</style>
