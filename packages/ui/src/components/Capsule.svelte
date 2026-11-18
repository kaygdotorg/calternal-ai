<script lang="ts" module>
  /** One choice in a Capsule. `label` is the accessible name even when only the icon shows. */
  export interface CapsuleItem {
    /** Stable id returned when the item is chosen. */
    id: string;
    /** The visible word; also the accessible name unless the item is shown as an icon only. */
    label: string;
    /** An optional icon shown with the word. */
    icon?: import("../icons").IconName;
    /** A small count or dot shown on the item (a waiting request). */
    badge?: number;
  }
</script>

<script lang="ts">
  // A segmented capsule that is also the tab bar: a pill of choices with one selected. `morph` shows only the selected item's label (the others shrink to their icon, on the spring: the mockup's bottom tab morph); `tall` is the phone's 49 px bottom bar tier.
  // `kind` is "tabs" (default: a tab list the app wires to a panel) or "radio" (a segmented choice, like the colour scheme). Keyboard: arrows move focus and select (automatic activation), Home and End jump. `label` names the group.
  import Glass from "./Glass.svelte";
  import Icon from "./Icon.svelte";
  import { adaptive } from "../lib/material";
  import { roving } from "../lib/roving";
  let {
    items,
    value = $bindable(),
    label,
    morph = false,
    tall = false,
    glass = true,
    kind = "tabs",
    onchange,
    class: klass,
  }: { items: CapsuleItem[]; value: string; label: string; morph?: boolean; tall?: boolean; glass?: boolean; kind?: "tabs" | "radio"; onchange?: (id: string) => void; class?: string } = $props();
  function pick(id: string) {
    if (id === value) return;
    value = id;
    onchange?.(id);
  }
  function onkey(e: KeyboardEvent) {
    // Roving moves focus; automatic activation selects what took it.
    queueMicrotask(() => {
      const el = document.activeElement as HTMLElement | null;
      const id = el?.dataset.id;
      if (id && ["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) pick(id);
    });
  }
</script>

<div class={["cap", klass]} class:morph class:tall role={kind === "tabs" ? "tablist" : "radiogroup"} aria-label={label} use:adaptive use:roving={{ selector: "[role=tab],[role=radio]" }} onkeydown={onkey}>
  {#if glass}<Glass />{/if}
  {#each items as it (it.id)}
    <button type="button" role={kind === "tabs" ? "tab" : "radio"} class="seg" data-id={it.id} aria-selected={kind === "tabs" ? value === it.id : undefined} aria-checked={kind === "radio" ? value === it.id : undefined} tabindex={value === it.id ? 0 : -1} aria-label={morph && value === it.id ? undefined : it.label} title={it.label} onclick={() => pick(it.id)}>
      {#if it.icon}<Icon name={it.icon} />{/if}
      <span class="txt" class:icononly={!!it.icon && morph && value !== it.id}>{it.label}</span>
      {#if it.badge}<b class="n" aria-label="{it.badge} waiting">{it.badge}</b>{/if}
    </button>
  {/each}
</div>

<style>
  .cap { position: relative; isolation: isolate; display: flex; align-items: center; gap: var(--s-1); padding: var(--s-2); border-radius: var(--r-pill); color: var(--ink-2); }
  .seg {
    position: relative; flex: 1 1 0; min-width: 0; display: inline-flex; align-items: center; justify-content: center; gap: var(--s-3);
    height: var(--pill-h); padding: 0 var(--seg-pad, var(--s-5)); border-radius: var(--r-pill);
    font-size: var(--text-ui); font-weight: var(--strong-weight); white-space: nowrap; overflow: hidden; color: var(--ink-2);
    transition: flex-grow var(--dur-slow) var(--spring), background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .seg:active { scale: var(--press-scale); }
  .seg[aria-selected="true"], .seg[aria-checked="true"] { background: rgb(var(--ink-rgb) / 0.11); color: var(--ink); }
  @media (hover: hover) and (pointer: fine) { .seg:not([aria-selected="true"]):not([aria-checked="true"]):hover { background: var(--hover); color: var(--ink); } }
  .cap.tall { height: var(--tier-cap); padding: var(--s-1); }
  .tall .seg { height: calc(var(--tier-cap) - 2 * var(--s-1)); }
  .morph .seg[aria-selected="true"] { flex-grow: 2.6; }
  /* The phone bar is five tabs wide: the selected one needs room for its icon and its whole word. */
  .tall.morph .seg[aria-selected="true"] { flex-grow: 3.8; }
  .txt { overflow: hidden; text-overflow: ellipsis; transition: opacity var(--dur-base) var(--ease-out), max-width var(--dur-slow) var(--spring); max-width: 12em; }
  .txt.icononly { opacity: 0; max-width: 0; }
  .n { min-width: var(--s-7); height: var(--s-7); padding: 0 var(--s-3); border-radius: var(--r-pill); background: var(--accent-base); color: #fff; font-size: var(--text-section); display: inline-grid; place-items: center; }
</style>
