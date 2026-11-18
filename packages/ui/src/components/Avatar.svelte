<script lang="ts">
  // A person's initial in a circle, the account avatar's look (accent tint, accent ink). Decorative: the name next to it, or the control around it, carries the meaning. `size` "md" is the control tier
  // (`--pill-h`, for an avatar that is a button's face); "sm" is a row's leading mark (`--ib-small`).
  let { name, size = "sm" }: { name: string; size?: "sm" | "md" } = $props();
  const initial = $derived([...name.trim()][0]?.toUpperCase() ?? "?");
</script>

<span class="av" class:md={size === "md"} aria-hidden="true">{initial}</span>

<style>
  .av { flex: none; display: grid; place-items: center; width: var(--ib-small); height: var(--ib-small); border-radius: 50%; font-size: var(--text-label); font-weight: var(--strong-weight); background: color-mix(in oklab, var(--accent-base) 22%, rgb(var(--tint) / 0.6)); color: var(--accent); }
  .md { width: var(--pill-h); height: var(--pill-h); font-size: var(--text-ui); }
</style>
