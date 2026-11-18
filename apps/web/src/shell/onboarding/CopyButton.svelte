<script lang="ts">
  // Copy a command: the button's own icon turns into the shared tick (drawn on, 200 ms), holds 0.9 s, and goes back (infraternal's copy feedback: nothing else on the page moves). A re-press retargets. The label changes with
  // it for a screen reader ("Copied"). If the browser refuses the clipboard the text stays selectable and `onfail` says so.
  import { IconButton, Tick } from "@calternal/ui";
  import { copyText } from "../../lib/format";
  let { text, label = "Copy the command", onfail }: { text: string; label?: string; onfail?: () => void } = $props();
  let done = $state(false), timer = 0;
  async function copy() {
    if (!(await copyText(text))) { onfail?.(); return; }
    done = true; clearTimeout(timer); timer = window.setTimeout(() => (done = false), 900 + 200);
  }
</script>

<span class="cb">
  <IconButton icon="copy" label={done ? "Copied" : label} onclick={copy} />
  <span class="tk" class:on={done} aria-hidden="true"><Tick checked={done} size="var(--s-8)" /></span>
</span>

<style>
  .cb { position: relative; display: inline-grid; }
  .tk { position: absolute; inset: 0; display: grid; place-items: center; pointer-events: none; opacity: 0; transition: opacity 150ms var(--ease-out); background: var(--paper); border-radius: var(--r-pill); }
  .tk.on { opacity: 1; transition-duration: 200ms; }
  @media (prefers-reduced-motion: reduce) { .tk { transition: opacity 1ms; } }
</style>
