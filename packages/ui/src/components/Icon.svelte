<script lang="ts">
  // One icon size and weight everywhere: `--glyph` and `--glyph-stroke` (tokens, tuned per form: 18 px on a desktop, 22 px on a phone).
  // Decorative by construction (aria-hidden): the control around an icon always carries its name.
  import { ICONS, type IconName } from "../icons";
  let { name, mini = false }: { name: IconName; mini?: boolean } = $props();
  // The dots of "more" are round caps on a zero-length line: they need a thicker stroke.
  const dots = $derived(name === "more");
  const px = $derived(mini ? "calc(13 * var(--icon-px))" : "var(--glyph)");
</script>

<svg style:width={px} style:height={px} style:stroke-width={dots ? "max(var(--glyph-stroke), 2.6)" : mini ? "2" : "var(--glyph-stroke)"} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" data-icon={name} aria-hidden="true" focusable="false"><path d={ICONS[name]} /></svg>

<style>
  svg { flex: none; display: block; }
</style>
