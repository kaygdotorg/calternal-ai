<script lang="ts">
  // The keyboard shortcut help and settings: every command with its chord, grouped, with a plain description of where it applies. Press Change, then the
  // new keys (Esc cancels); a chord already in use says so and is refused. Opened with Cmd or Ctrl + / or ?, from the account sheet, or from the palette.
  import { Button, Sheet } from "@calternal/ui";
  import { shortcuts as S } from "../lib/shortcuts.svelte";
  import ShortcutsList from "./ShortcutsList.svelte";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
</script>

<Sheet {open} label="Keyboard shortcuts" onclose={() => { S.capturing = null; S.problem = ""; onclose(); }} side="right">
  <div class="in">
    <header><h1>Keyboard shortcuts</h1><Button variant="quiet" text="Reset all" onclick={() => S.resetAll()} /></header>
    <ShortcutsList />
  </div>
</Sheet>

<style>
  .in { display: grid; gap: var(--s-5); padding: var(--s-6); }
  header { display: flex; align-items: center; justify-content: space-between; min-height: var(--bar-h); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
</style>
