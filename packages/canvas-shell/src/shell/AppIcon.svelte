<script lang="ts">
  // An app's glyph: a name from the design system's icon set (`packages/ui` ICONS, which now holds every glyph the mockup lanes drew). Kept as one small wrapper so a name the set lacks draws nothing rather than failing.
  import { Icon, ICONS, type IconName } from "@calternal/ui";
  let { name, inline = false }: { name: string; inline?: boolean } = $props();
</script>

{#if name in ICONS}<Icon name={name as IconName} {inline} />{/if}
