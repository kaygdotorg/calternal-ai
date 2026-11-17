<script lang="ts">
  // A card: a landmark region with a required `label`.
  //   pane       the shell's middle card. Its material is two layers of its own, never a backdrop-filter over a large area: the wallpaper's
  //              second, much blurrier copy (made once when the wallpaper was chosen, `--wall-card`), then a tint and a dim
  //              (the solved veil, `--veil-c`). With no wallpaper the background mesh stands in for the copy. This is the owner's
  //              calternal-ui card, and costs one image paint instead of a blur per frame.
  //   solid      sits on paper.
  //   attention  the tinted card that asks for the user's attention.
  import type { Snippet } from "svelte";
  let {
    label,
    variant = "pane",
    children,
    class: klass,
    ...rest
  }: { label: string; variant?: "pane" | "solid" | "attention"; children: Snippet; class?: string } & Record<string, unknown> = $props();
</script>

<section class={["card", klass]} data-variant={variant} aria-label={label} {...rest}>
  {@render children()}
</section>

<style>
  .card { position: relative; isolation: isolate; border-radius: var(--r-pane); min-width: 0; min-height: 0; }
  .card[data-variant="pane"] { box-shadow: var(--shadow-card); overflow: clip; }
  .card[data-variant="pane"]::before, .card[data-variant="pane"]::after { content: ""; position: absolute; inset: 0; pointer-events: none; }
  .card[data-variant="pane"]::before { z-index: -2; background: var(--wall-card, var(--wall-fallback, var(--backdrop))) center / cover no-repeat fixed; }
  .card[data-variant="pane"]::after { z-index: -1; background: var(--veil-c, linear-gradient(rgb(var(--tint) / 0.9) 0 0)); }
  .card[data-variant="solid"] { background: var(--paper); box-shadow: var(--shadow-card); }
  .card[data-variant="attention"] { background: color-mix(in oklab, var(--accent-base) 12%, rgb(var(--tint) / 0.6)); border-radius: var(--r-card); box-shadow: inset 0 0 1px 1px color-mix(in oklab, var(--accent-base) 22%, transparent); }
</style>
