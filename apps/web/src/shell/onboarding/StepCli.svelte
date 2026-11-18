<script lang="ts">
  // Coding step two: the command line. Two commands for the computer you code on: install the `calternal` command, and sign it in to this server. Nothing here runs them; each has a copy button (the icon turns into the tick).
  import { Button } from "@calternal/ui";
  import CopyButton from "./CopyButton.svelte";
  let { ondone }: { ondone: () => void } = $props();
  const origin = location.origin;
  const cmds = [["Install it", "curl -fsSL " + origin + "/install-cli.sh | sh"], ["Sign it in to this server", "calternal login --server " + origin]];
</script>

<div class="in">
  <p class="note">The <code>calternal</code> command starts threads, reads their files and works with your repositories from a terminal. Run these two on the computer you code on:</p>
  {#each cmds as [what, cmd]}
    <div class="one"><span class="what">{what}</span><div class="cmd"><code>{cmd}</code><CopyButton text={cmd} label="Copy: {what}" /></div></div>
  {/each}
  <div class="go"><Button variant="primary" text="Continue" onclick={ondone} /></div>
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .one { display: grid; gap: var(--s-2); }
  .what { font-size: var(--text-label); color: var(--ink-2); }
  .cmd { display: flex; align-items: center; gap: var(--s-4); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); }
  code { flex: 1; min-width: 0; overflow-wrap: anywhere; font-family: var(--font-mono); font-size: var(--text-label); }
  .go { display: flex; gap: var(--s-4); }
</style>
