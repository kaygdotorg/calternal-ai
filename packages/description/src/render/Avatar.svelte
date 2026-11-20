<script lang="ts">
  // The avatar mark of a row (DRAFT icon `{avatar: {label, image?}}`): a round picture the host authorised, else the initials of the label on a colour taken from the row's id (the same colour every time, a soft tint of the
  // accent's lightness so ink stays readable in both schemes). Decorative: the row names the person; `label` is only what the initials are made from.
  import RowImage from "./RowImage.svelte";
  let { label, id, image, size = "var(--avatar, 28px)" }: { label: string; id: string; image?: string; size?: string } = $props();
  const initials = $derived((label.match(/[\p{L}\p{N}]+/gu) ?? []).slice(0, 2).map((w) => w[0]!.toUpperCase()).join("") || "?");
  const hue = $derived([...id].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));
</script>

<span class="av" style:--hue={hue} style:--sz={size} aria-hidden="true" data-avatar>{#if image}<RowImage ref={image} snippet={undefined} />{:else}{initials}{/if}</span>

<style>
  .av { flex: none; display: inline-grid; place-items: center; width: var(--sz); height: var(--sz); border-radius: 50%; overflow: hidden; font-size: calc(var(--sz) * 0.38); font-weight: var(--strong-weight); letter-spacing: 0.02em;
    color: var(--ink); background: color-mix(in oklab, oklch(0.74 0.08 var(--hue)) 55%, rgb(var(--tint) / 1)); box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); }
</style>
