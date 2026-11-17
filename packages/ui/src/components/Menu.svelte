<script lang="ts" module>
  /** One entry of a Menu. `label` is its name. */
  export interface MenuItem {
    id: string;
    label: string;
    sub?: string;
    icon?: import("../icons").IconName;
    checked?: boolean;
  }
</script>

<script lang="ts">
  // A popover list that grows from a trigger the caller owns (the caller sets `aria-expanded` and `aria-haspopup` on it and passes `open`).
  // Items are radio-style menu items when any is `checked`. Arrows move focus, Enter picks, Escape and an outside press close it and
  // return focus to `trigger`. `label` names the menu.
  import Glass from "./Glass.svelte";
  import Icon from "./Icon.svelte";
  import { adaptive } from "../lib/material";
  import { ready } from "../lib/ready";
  import { roving } from "../lib/roving";
  let {
    open,
    items,
    label,
    onpick,
    onclose,
    trigger,
    placement = "top",
  }: { open: boolean; items: MenuItem[]; label: string; onpick: (id: string) => void; onclose: () => void; trigger?: HTMLElement | null; placement?: "top" | "bottom" } = $props();
  let box = $state<HTMLElement>();
  const radio = $derived(items.some((i) => i.checked !== undefined));
  $effect(() => {
    if (!open || !box) return;
    (box.querySelector<HTMLElement>('[aria-checked="true"]') ?? box.querySelector<HTMLElement>("[role^=menuitem]"))?.focus();
    const away = (e: PointerEvent) => {
      if (!box!.contains(e.target as Node) && !trigger?.contains(e.target as Node)) onclose();
    };
    addEventListener("pointerdown", away, true);
    return () => removeEventListener("pointerdown", away, true);
  });
  function close() {
    onclose();
    trigger?.focus();
  }
</script>

{#if open}
  <div bind:this={box} class="menu {placement}" role="menu" aria-label={label} tabindex="-1" use:adaptive use:ready use:roving={{ selector: "[role^=menuitem]", axis: "y" }} onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), close())}>
    <Glass dense />
    {#each items as it (it.id)}
      <button type="button" class="it" role={radio ? "menuitemradio" : "menuitem"} aria-checked={radio ? !!it.checked : undefined} tabindex="-1" onclick={() => { onpick(it.id); close(); }}>
        {#if it.icon}<Icon name={it.icon} />{/if}
        <span class="tx"><span>{it.label}</span>{#if it.sub}<small>{it.sub}</small>{/if}</span>
        {#if it.checked}<Icon name="check" />{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .menu { position: absolute; z-index: 30; isolation: isolate; min-width: 15rem; max-width: 22rem; padding: var(--s-2); border-radius: var(--r-card); display: grid; gap: var(--s-1); overscroll-behavior: contain; --origin: bottom left; }
  .top { bottom: calc(100% + var(--s-3)); left: 0; }
  .bottom { top: calc(100% + var(--s-3)); left: 0; --origin: top left; }
  .it { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: var(--row-h); padding: var(--s-3) var(--s-4); border-radius: calc(var(--r-card) - var(--s-2)); text-align: start; color: var(--ink); }
  .tx { flex: 1; min-width: 0; display: grid; }
  small { font-size: var(--text-label); color: var(--ink-3); }
  .it:focus-visible { outline-offset: -2px; }
  @media (hover: hover) and (pointer: fine) { .it:hover { background: var(--hover); } }
  .it[aria-checked="true"] { color: var(--accent); }
</style>
