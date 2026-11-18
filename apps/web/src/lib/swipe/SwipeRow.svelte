<script lang="ts" module>
  /** One swipe button. `full`: a long swipe runs it (only the outer one of a side). `leaves`: the row goes away when it runs. */
  export interface SwipeAction {
    /** Unique within the row; the key the row uses for the button. */
    id: string;
    /** The button's text, also its accessible name. */
    label: string;
    /** The button's icon. */
    icon: import("@calternal/ui").IconName;
    /** The button's colour role: `accent` for the main action, `danger` for removal. */
    tone: "accent" | "plain" | "danger";
    /** Only on the outer button of a side: a long swipe past the commit point runs this button. */
    full?: boolean;
    /** The row leaves the list when this runs (archive, delete), so a long swipe keeps it out until `run` settles. */
    leaves?: boolean;
    /** Runs when the button is tapped or a long swipe commits it. Return a promise when it takes a while: a row a long swipe threw out waits for it. */
    run: (el: HTMLElement | undefined) => void | Promise<unknown>;
  }
</script>

<script lang="ts">
  // A row with swipe actions (touch, and a two-finger sideways scroll on a trackpad), ported from the existing app's `rows/RowActions.svelte` (#400). It has no listeners of its own: the list's
  // scroller owns them (`use:swipeRows`, ./rows.ts), so this only draws the buttons and says what they do. `lead` shows from the left edge, `trail` from the right; slot 0 is the outer
  // edge, the one a long swipe runs. The buttons are drawn on the first touch, never for rows nobody touches. The look is the existing app's: soft tiles, icon over label (on a desktop, whose rows are
  // shorter, the icon alone), the outer one stretching and turning solid as a long swipe takes over. `busy` says a hold on the row matters (a drag has begun): the row then never claims a swipe.
  import type { Snippet } from "svelte";
  import { Icon } from "@calternal/ui";
  import { registerRow } from "./rows";
  import { shutOpenRow } from "./open";
  import type { Side, SideConf } from "./row";

  let { lead = [], trail = [], busy, children }: { lead?: SwipeAction[]; trail?: SwipeAction[]; busy?: () => boolean; children: Snippet } = $props();

  let node = $state<HTMLElement>();
  let trayOn = $state(false);
  const conf = (a: SwipeAction[]): SideConf => ({ n: a.length, full: !!a[0]?.full, leaves: !!a[0]?.leaves });

  /** A long swipe settles its own row; a tap on a button shuts the open one first. */
  function run(a: SwipeAction, via: "swipe" | "full") {
    if (via === "swipe") shutOpenRow();
    return a.run(node);
  }

  $effect(() => {
    if (!node) return;
    return registerRow(node, {
      ontouch: () => (trayOn = true),
      off: () => false,
      config: () => ({
        busy: () => !!busy?.(),
        lead: conf(lead),
        trail: conf(trail),
        onfull: (s: Side) => { const a = (s > 0 ? lead : trail)[0]; return a && run(a, "full"); },
      }),
    });
  });
</script>

<div class="ra" bind:this={node}>
  {#if trayOn}
    <div class="ra-tray">
      {#each [["lead", lead], ["trail", trail]] as const as [side, acts] (side)}
        <div class="ra-side {side}">
          {#each acts as a, i (a.id)}
            <button type="button" class="ra-act" class:outer={i === 0} data-tone={a.tone} style="--i: {i}" tabindex="-1" aria-label={a.label} onclick={() => void run(a, "swipe")}>
              <Icon name={a.icon} />
              <span class="lbl">{a.label}</span>
            </button>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
  <div class="ra-content">{@render children()}</div>
</div>

<style>
  .ra { position: relative; border-radius: var(--r-select); touch-action: pan-y; -webkit-touch-callout: none; -webkit-tap-highlight-color: transparent; }
  /* The row is never clipped by its own box: the moving card slides out past the list's edge, and the list's scroller cuts it there. Only the row that moves gets its own layer. */
  :global(.ra:is(.ra-swiping, .ra-open) > .ra-content) { will-change: transform; }
  .ra-content { position: relative; z-index: 1; border-radius: inherit; }
  /* The tray: soft tiles, inset from the row, icon over label. Its own layout box: the outer pill's width, written every frame, never reaches the list. */
  .ra-tray { position: absolute; inset: 4px; contain: layout paint; border-radius: calc(var(--r-select) - 2px); overflow: clip; pointer-events: none; }
  .ra-side { position: absolute; inset: 0; visibility: hidden; }
  :global(.ra[data-side="lead"]) .ra-side.lead, :global(.ra[data-side="trail"]) .ra-side.trail { visibility: visible; }
  :global(.ra.ra-open) .ra-act { pointer-events: auto; }
  .ra-act {
    position: absolute; top: 0; bottom: 0; width: 56px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 3px; padding: 0; border-radius: 14px;
    background: var(--t-soft); color: var(--t-ink); font-size: 10px; letter-spacing: -0.01em; font-weight: var(--strong-weight); line-height: 12px;
    transition: opacity 120ms var(--ease-out), scale 160ms var(--ease-out), background-color 140ms var(--ease-out), color 140ms var(--ease-out);
  }
  :global(.ra.ra-swiping) .ra-act { transition: none; }
  .lead .ra-act { left: calc(var(--i) * 64px); }
  .trail .ra-act { right: calc(var(--i) * 64px); }
  .lbl { max-width: 100%; text-align: center; overflow: hidden; white-space: nowrap; }
  /* A desktop row is 32 px tall: the tile (row less 8) has room for the icon only; the name stays as the button's accessible name. */
  @media (hover: hover) and (pointer: fine) { .lbl { display: none; } .ra-act { border-radius: var(--r-select); } }
  [data-tone="accent"] { --t-soft: var(--accent-soft); --t-ink: var(--accent); --t-solid: var(--accent); --t-on: #fff; }
  [data-tone="plain"] { --t-soft: rgb(var(--ink-rgb) / 0.1); --t-ink: var(--ink-2); --t-solid: var(--ink-2); --t-on: var(--paper); }
  [data-tone="danger"] { --t-soft: color-mix(in oklab, var(--danger) 15%, transparent); --t-ink: var(--danger); --t-solid: var(--danger); --t-on: var(--paper); }
  /* The long swipe: the outer pill stretches with the finger (row.ts sets its width and --s) and turns solid as it takes over the gap; the others slide under the row, shrink and fade. */
  .ra-act.outer { overflow: hidden; color: color-mix(in oklab, var(--t-ink), var(--t-on) calc(var(--s, 0) * 100%)); }
  .ra-act.outer::before { content: ""; position: absolute; inset: 0; border-radius: inherit; background: var(--t-solid); opacity: var(--s, 0); transition: opacity 240ms var(--ease-out); }
  :global(.ra.ra-swiping) .ra-act.outer::before { transition: none; }
  .ra-act > :global(*) { position: relative; }
</style>
