<script lang="ts">
  // The header's line in a pill beside the sidebar button: the page's title and then the section being read, as each passes the header line (see header/track.ts). Hidden while the
  // page's own title is readable. It grows in and fades out on the motion tokens (`--dur-fast` for the text swap, `--dur-base` for appearing), any change interrupts the one before
  // it (they are CSS transitions), and under reduced motion the tokens are 1 ms. It repeats the page's own text, so assistive technology skips it.
  import { fade } from "svelte/transition";
  import { chromeHeader } from "../header/header.svelte";
  import { tokenMs } from "../lib/motion";
  import Panel from "./Panel.svelte";
  let last = $state("");
  $effect(() => { if (chromeHeader.line) last = chromeHeader.line; });
</script>

<div class="hdr" data-on={chromeHeader.line ? "" : undefined} aria-hidden="true">
  <Panel class="hp" radius="pill">
    {#key chromeHeader.key || chromeHeader.pinned}
      <span class="t" in:fade={{ duration: chromeHeader.instant ? 0 : tokenMs("--dur-fast", 150) }}>{last}</span>
    {/key}
  </Panel>
</div>

<style>
  .hdr { max-width: 100%; min-width: 0; opacity: 0; scale: 0.96; pointer-events: none; transition: opacity var(--dur-fast) var(--ease-out), scale var(--dur-base) var(--ease-out); }
  .hdr[data-on] { opacity: 1; scale: 1; }
  .hdr :global(.hp) { display: flex; align-items: center; height: var(--pill-h); padding: 0 var(--s-5); min-width: 0; max-width: 100%; }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
