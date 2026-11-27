<script lang="ts">
  // A round picture stand-in: the initials of a name on a colour taken from it (the same colour every time). Fixture only: a real one shows the person's or group's picture first.
  let { name, size = "var(--avatar, 28px)" }: { name: string; size?: string } = $props();
  const initials = $derived((name.match(/[\p{L}\p{N}]+/gu) ?? []).slice(0, 2).map((w) => w[0]!.toUpperCase()).join("") || "?");
  const hue = $derived([...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));
</script>

<span class="av" style:--hue={hue} style:--sz={size} aria-hidden="true"><span class="in">{initials[0]}<span class="b">{initials.slice(1)}</span></span></span>

<style>
  .av { flex: none; display: inline-grid; place-items: center; width: var(--sz); height: var(--sz); border-radius: 50%; container-type: size; font-weight: var(--strong-weight); color: var(--ink);
    background: color-mix(in oklab, oklch(0.88 0.06 var(--hue)) 62%, rgb(var(--tint) / 1)); box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); }
  /* the initials are drawn from the type scale only: one letter at the smallest size, two at 11, 13 for a medium picture, 22 for a large one */
  .in { font-size: var(--text-section); line-height: 1; }
  @container (max-width: 21px) { .b { display: none; } }
  @container (min-width: 35px) { .in { font-size: var(--text-ui); } }
  @container (min-width: 57px) { .in { font-size: var(--text-title); } }
  :global(html[data-scheme="dark"]) .av { background: color-mix(in oklab, oklch(0.42 0.07 var(--hue)) 66%, rgb(var(--tint) / 1)); }
</style>
