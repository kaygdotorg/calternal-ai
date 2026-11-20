<script lang="ts">
  // A person's stand-in picture: two initials on a colour derived from the name (the mockup's `Av`), since packages/ui's
  // Avatar draws every person in the accent tint and a timeline needs people told apart. A real host sends the picture file.
  let { name, size = 28 }: { name: string; size?: number } = $props();
  const initials = $derived((name.match(/[\p{L}\p{N}]+/gu) ?? []).slice(0, 2).map((w) => w[0]!.toUpperCase()).join("") || "?");
  const hue = $derived([...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));
</script>

<span class="av" style:--hue={hue} style:--sz="{size}px" aria-hidden="true">{initials}</span>

<style>
  .av { flex: none; display: inline-grid; place-items: center; width: var(--sz); height: var(--sz); border-radius: 50%; font-size: calc(var(--sz) * 0.38); font-weight: var(--strong-weight); color: var(--ink);
    background: color-mix(in oklab, oklch(0.74 0.08 var(--hue)) 55%, rgb(var(--tint) / 1)); box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); }
</style>
