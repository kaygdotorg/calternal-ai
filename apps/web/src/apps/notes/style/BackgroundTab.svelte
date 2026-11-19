<script lang="ts">
  // Background: the picture behind the note (the existing app's backdrop builder: None, Solid, Gradient, Image). A note's own backdrop needs the shell's wallpaper layer to follow the open note, which is
  // the shell's, not the editor's; until that hook exists this tab says so and writes nothing (docs/notes-style.md, "left").
</script>

<section aria-labelledby="st-bg">
  <h3 class="section-label" id="st-bg">Background</h3>
  <p class="soft">The picture behind a note follows the app's wallpaper for now. A note's own backdrop (none, a colour, a gradient or a picture) arrives with the shell's hook for it.</p>
</section>

<style>
  section { display: grid; gap: var(--s-3); }
  .soft { font-size: var(--text-ui); color: var(--ink-3); }
</style>
