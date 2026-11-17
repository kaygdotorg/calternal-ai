<script lang="ts">
  // The Keyboard page: every shortcut with Change and Reset, and the account's shortcuts file as it would be stored (docs/settings.md: settings are files).
  import ShortcutsList from "../../shell/ShortcutsList.svelte";
</script>

<div class="p"><ShortcutsList file /></div>

<style>
  .p { padding: var(--s-5) var(--s-6); }
</style>
