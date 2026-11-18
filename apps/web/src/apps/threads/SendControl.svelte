<script lang="ts">
  // The composer's one round control and its states (docs/composer.md): the same circle in every state, the height of the line (`--pill-h`), so the field's first line and the control sit on one centre.
  //   idle      nothing to send: a quiet circle (the ink at 10 %) with a quiet arrow, disabled
  //   ready     something to send: the accent circle, a white arrow
  //   busy      files are still uploading: the quiet circle with a spinner, disabled
  //   steer     a turn is running and there is text: the accent circle (the message steers the turn)
  //   stop      a turn is running and the box is empty: the ink circle with a solid rounded square in the paper colour (ChatGPT's and bb's stop), the weight of the glyph being the point
  import { Spinner, tip } from "@calternal/ui";
  let { state, label, onclick }: { state: "idle" | "ready" | "busy" | "steer" | "stop"; label: string; onclick: () => void } = $props();
  const off = $derived(state === "idle" || state === "busy");
</script>

<button type="button" class="go" data-state={state} aria-label={label} disabled={off} use:tip={{ label }} {onclick}>
  {#if state === "stop"}
    <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="4.5" fill="currentColor" /></svg>
  {:else if state === "busy"}
    <Spinner />
  {:else}
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M12 19V5M5.5 11.5 12 5l6.5 6.5" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
  {/if}
</button>

<style>
  .go { flex: none; display: inline-grid; place-items: center; width: var(--pill-h); height: var(--pill-h); border-radius: 50%; transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .go:active:not(:disabled) { scale: var(--press-scale); }
  .go[data-state="idle"], .go[data-state="busy"] { background: rgb(var(--ink-rgb) / 0.1); color: var(--ink-3); cursor: default; }
  .go[data-state="ready"], .go[data-state="steer"] { background: var(--accent-base); color: #fff; }
  .go[data-state="stop"] { background: var(--ink); color: var(--paper); }
</style>
