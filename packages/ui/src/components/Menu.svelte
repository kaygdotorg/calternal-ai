<script lang="ts" module>
  /** One entry of a Menu. `label` is its name. */
  export interface MenuItem {
    /** Stable id returned when the item is picked. */
    id: string;
    /** The visible text and accessible name. */
    label: string;
    /** A second, quieter line. */
    sub?: string;
    /** An optional leading icon. */
    icon?: import("../icons").IconName;
    /** Shows a check mark: this is the current choice (for a menu that picks one value). */
    checked?: boolean;
    /** A destructive entry (drawn in the danger colour). */
    danger?: boolean;
    /** Shown dimmed and not pickable. */
    disabled?: boolean;
  }
</script>

<script lang="ts">
  // A menu on a Popover (placement, portal and material are the Popover's); on a touch screen it is an inset sheet from the bottom instead, with rows of the native menu height (42). Items are radio-style when any is `checked`. Arrows move focus, Enter picks,
  // Escape and an outside press close it. `search` adds an inline field at the top that filters by label and sub text (Down moves into the list).
  // Anchored to `trigger` (a button) or to `anchor` (a point, for a context menu). `label` names the menu.
  import Icon from "./Icon.svelte";
  import Popover from "./Popover.svelte";
  import { textField } from "../lib/textfield";
  import Sheet from "./Sheet.svelte";
  import { currentForm } from "../lib/theme";
  import { roving } from "../lib/roving";
  import type { Anchor } from "../lib/place";
  let {
    open,
    items,
    label,
    onpick,
    onclose,
    trigger,
    anchor,
    search = false,
    morph = false,
    placement = "top",
  }: { open: boolean; items: MenuItem[]; label: string; onpick: (id: string) => void; onclose: () => void; trigger?: HTMLElement | null; anchor?: Anchor | null; search?: boolean; morph?: boolean; placement?: "top" | "bottom" } = $props();
  let q = $state("");
  let list = $state<HTMLElement>();
  const radio = $derived(items.some((i) => i.checked !== undefined));
  const shown = $derived(search && q.trim() ? items.filter((i) => `${i.label} ${i.sub ?? ""}`.toLowerCase().includes(q.trim().toLowerCase())) : items);
  $effect(() => {
    if (!open) return void (q = "");
    queueMicrotask(() => {
      if (search) list?.parentElement?.querySelector<HTMLInputElement>("input")?.focus();
      else (list?.querySelector<HTMLElement>('[aria-checked="true"]') ?? list?.querySelector<HTMLElement>("[role^=menuitem]"))?.focus();
    });
  });
  function close() {
    onclose();
    trigger?.focus();
  }
  function fromSearch(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      list?.querySelector<HTMLElement>("[role^=menuitem]")?.focus();
    } else if (e.key === "Enter" && shown[0] && !shown[0].disabled) {
      e.preventDefault();
      onpick(shown[0].id);
      close();
    }
  }
</script>

{#snippet content()}
  {#if search}
    <div class="s" data-ring-host>
      <Icon inline name="search" />
      <input type="search" use:textField={{ kind: "search", cancels: true }} placeholder="Search" aria-label="Search {label}" bind:value={q} autocomplete="off" onkeydown={fromSearch} />
    </div>
  {/if}
  <div bind:this={list} class="menu" role={search ? "menu" : "presentation"} aria-label={search ? label : undefined} use:roving={{ selector: "[role^=menuitem]", axis: "y" }}>
    {#each shown as it (it.id)}
      <button type="button" class="it" class:danger={it.danger} disabled={it.disabled} role={radio ? "menuitemradio" : "menuitem"} aria-checked={radio ? !!it.checked : undefined} tabindex="-1" onclick={() => { onpick(it.id); close(); }}>
        {#if it.icon}<Icon name={it.icon} />{/if}
        <span class="tx"><span>{it.label}</span>{#if it.sub}<small>{it.sub}</small>{/if}</span>
        {#if it.checked}<Icon inline name="check" />{/if}
      </button>
    {:else}
      <p class="none">Nothing matches.</p>
    {/each}
  </div>
{/snippet}

{#if currentForm() === "touch"}
  <Sheet {open} {label} onclose={onclose}><div class="sh" role={search ? "group" : "menu"} aria-label={label}>{@render content()}</div></Sheet>
{:else}
  <Popover {open} anchor={anchor ?? trigger} {label} role={search ? "dialog" : "menu"} prefer={placement} {morph} {trigger} onclose={onclose}>{@render content()}</Popover>
{/if}

<style>
  .sh { padding: var(--s-2) 0; } .sh .menu { min-width: 0; max-width: none; } .sh .it { min-height: var(--menu-row-h, 42px); }
  .menu { min-width: 15rem; max-width: 22rem; padding: var(--s-2); display: grid; gap: var(--s-1); }
  .s { display: flex; align-items: center; gap: var(--s-3); margin: var(--s-2) var(--s-2) 0; padding: 0 var(--s-4); min-height: var(--pill-h); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink-3); }
  .s input { flex: 1; min-width: 0; border: 0; background: none; color: var(--ink); font-size: var(--text-ui); }
  .s input:focus, .s input:focus-visible { outline: none; }
  .it { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: var(--row-h); padding: var(--s-3) var(--s-4); border-radius: var(--r-select); text-align: start; color: var(--ink); }
  .it:disabled { opacity: 0.4; }
  .it.danger { color: var(--danger); }
  .tx { flex: 1; min-width: 0; display: grid; }
  small { font-size: var(--text-label); color: var(--ink-3); }
  @media (hover: hover) and (pointer: fine) { .it:not(:disabled):hover { background: var(--hover); } }
  .it[aria-checked="true"] { color: var(--accent); }
  .none { padding: var(--s-4); color: var(--ink-3); font-size: var(--text-label); }
</style>
