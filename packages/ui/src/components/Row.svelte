<script lang="ts">
  // A list row: icon or leading content, a title, an optional quiet line, trailing content. A link when it has an `href`, else a button.
  // `meta` replaces the quiet line with content of its own (a state icon and word, say). `title` is required and is the accessible name. `tall` is the settings-row height (44 desktop, 52 phone). A selected row is the accent tint (concentric with its pane: --r-select).
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import type { IconName } from "../icons";
  let {
    title,
    sub,
    meta,
    icon,
    href,
    selected = false,
    depth = 0,
    tall = false,
    leading,
    trailing,
    onclick,
    class: klass,
  }: { title: string; sub?: string; meta?: Snippet; icon?: IconName; href?: string; selected?: boolean; depth?: number; tall?: boolean; leading?: Snippet; trailing?: Snippet; onclick?: (e: MouseEvent) => void; class?: string } = $props();
</script>

{#snippet inner()}
  {#if leading}{@render leading()}{:else if icon}<span class="ic"><Icon name={icon} /></span>{/if}
  <span class="txt"><span class="t">{title}</span>{#if meta}<span class="s">{@render meta()}</span>{:else if sub}<span class="s">{sub}</span>{/if}</span>
  {#if trailing}<span class="tr">{@render trailing()}</span>{/if}
{/snippet}

{#if href}
  <a class={["row", klass]} class:tall {href} aria-current={selected ? "page" : undefined} style:--depth={depth} {onclick}>{@render inner()}</a>
{:else}
  <button type="button" class={["row", klass]} class:tall aria-current={selected ? "true" : undefined} style:--depth={depth} {onclick}>{@render inner()}</button>
{/if}

<style>
  .row {
    position: relative; display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: var(--row-h); text-align: start;
    padding: 0 var(--s-5) 0 calc(var(--s-5) + var(--depth, 0) * var(--s-7)); border-radius: var(--r-select); color: var(--ink); font-size: var(--text-ui);
    transition: scale var(--dur-fast) var(--ease-out);
  }
  .row.tall { min-height: var(--row-h-settings); }
  .row:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .row:not([aria-current]):hover { background: var(--hover); } }
  .row[aria-current], .row[data-cursor] { background: var(--accent-soft); }
  .ic { flex: none; display: grid; color: var(--ink-2); }
  .row[aria-current] .ic, .row[data-cursor] .ic { color: var(--accent); }
  .txt { flex: 1; min-width: 0; display: grid; }
  .t, .s { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .s { display: flex; align-items: center; gap: var(--s-2); }
  .s { font-size: var(--text-label); color: var(--ink-3); }
  .tr { flex: none; display: inline-flex; align-items: center; gap: 6px; font-size: var(--text-label); color: var(--ink-3); }
</style>
