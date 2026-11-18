<script lang="ts">
  // Two tiers, no other size: `--glyph` for controls (18 px desktop, 22 px phone) and `--glyph-inline` (`inline`) beside text (14 and 18), each with its
  // own stroke so the rendered line is the same weight in both. One set, drawn on a 24 grid with round caps, in `currentColor`.
  // Decorative by construction (aria-hidden): the control around an icon always carries its name.
  import { ICONS, type IconName } from "../icons";
  // `solid` fills the shape with the ink (a stop square, a record dot); `bold` draws the line one third heavier (a glyph that carries a control by itself, the send arrow). Both change only the drawing, never the size.
  let { name, inline = false, solid = false, bold = false }: { name: IconName; inline?: boolean; solid?: boolean; bold?: boolean } = $props();
  const small = $derived(inline);
  // The dots of "more" are round caps on a zero-length line: they need a thicker stroke.
  const dots = $derived(name === "more");
  const px = $derived(small ? "var(--glyph-inline)" : "var(--glyph)");
</script>

<svg style:width={px} style:height={px} style:stroke-width={dots ? "max(var(--glyph-stroke), 2.6)" : bold ? `calc(${small ? "var(--glyph-inline-stroke)" : "var(--glyph-stroke)"} * 1.33)` : small ? "var(--glyph-inline-stroke)" : "var(--glyph-stroke)"} viewBox="0 0 24 24" fill={solid ? "currentColor" : "none"} stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" data-icon={name} aria-hidden="true" focusable="false"><path d={ICONS[name]} /></svg>

<style>
  svg { flex: none; display: block; }
</style>
