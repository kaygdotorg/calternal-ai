<script lang="ts">
  // Step two: a look. The same gallery as Appearance; choosing one is the answer (it applies at once behind this card, so what is chosen is seen) and the step confirms and moves on. Skipping keeps the default.
  import PresetGallery from "../../appearance/PresetGallery.svelte";
  let { ondone }: { ondone: () => void } = $props();
</script>

<div class="in">
  <p class="note">A preset sets the backdrop and the colours together. It shows behind this card as you choose; everything can be changed later in Appearance.</p>
  <PresetGallery onpick={() => ondone()} />
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
</style>
