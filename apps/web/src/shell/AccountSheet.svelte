<script lang="ts">
  // The account sheet: who is signed in, and the way into settings. Fixture identity; sign-in and passkeys are the server's.
  import { Row, Sheet } from "@calternal/ui";
  import { href } from "../lib/route.svelte";
  import { shell } from "./shell.svelte";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
</script>

<Sheet {open} label="Account" {onclose}>
  <div class="in">
    <header><span class="av" aria-hidden="true">K</span><div><h2>Kay</h2><p>Personal space</p></div></header>
    <Row tall title="Keyboard shortcuts" sub="See and change them" icon="terminal" onclick={() => { onclose(); shell.helpOpen = true; }} />
    <Row tall title="Appearance" sub="Wallpaper, glass, colour, fonts" icon="settings" href={href("/settings/appearance")} onclick={onclose} />
  </div>
</Sheet>

<style>
  .in { display: grid; gap: var(--s-5); padding: var(--s-6); }
  header { display: flex; align-items: center; gap: var(--s-5); }
  .av { width: var(--s-10); height: var(--s-10); border-radius: 50%; display: grid; place-items: center; background: var(--accent-soft); color: var(--accent); font-size: var(--text-heading); font-weight: var(--strong-weight); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  p { color: var(--ink-3); font-size: var(--text-label); }
</style>
