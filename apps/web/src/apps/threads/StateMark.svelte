<script lang="ts">
  // A thread's state, said once and in one form: an icon (rows, where it sits at the right) or a word (the title pill), never both.
  // Running is the spinner, waiting a bell, failed a warning. Idle and finished threads show nothing. The word is always the accessible name.
  import { Icon, Spinner } from "@calternal/ui";
  import type { ThreadState } from "./model";
  let { state, form = "icon" }: { state: ThreadState; form?: "icon" | "word" } = $props();
  const SAY = { running: "Running", waiting: "Waiting for you", failed: "Failed" } as const;
  const word = $derived(state === "running" || state === "waiting" || state === "failed" ? SAY[state] : "");
</script>

{#if word}
  <span class="mark {state}" role="img" aria-label={word}>
    {#if form === "word"}<span aria-hidden="true">{word}</span>{:else if state === "running"}<Spinner />{:else}<Icon inline name={state === "waiting" ? "bell" : "warning"} />{/if}
  </span>
{/if}

<style>
  .mark { display: inline-flex; align-items: center; font-size: var(--text-label); font-weight: var(--strong-weight); white-space: nowrap; color: var(--c); }
  .running { --c: var(--ink-2); } .waiting { --c: var(--ink); } .failed { --c: var(--danger); }
</style>
