<script lang="ts">
  // A thread's state, said once and only when it means something: running, waiting for you, failed. Each is an icon and a word (the word can be
  // dropped where there is no room, `compact`; the icon never carries it alone for assistive tech: the word is always in the accessible name).
  // Idle and finished threads show nothing.
  import { Icon, Spinner } from "@calternal/ui";
  import type { ThreadState } from "./model";
  let { state, compact = false }: { state: ThreadState; compact?: boolean } = $props();
  const SAY = { running: "Running", waiting: "Waiting for you", failed: "Failed" } as const;
  const word = $derived(state === "running" || state === "waiting" || state === "failed" ? SAY[state] : "");
</script>

{#if word}
  <span class="mark {state}" role="img" aria-label={word}>
    {#if state === "running"}<Spinner />{:else}<Icon inline name={state === "waiting" ? "bell" : "warning"} />{/if}
    {#if !compact}<span class="w" aria-hidden="true">{word}</span>{/if}
  </span>
{/if}

<style>
  .mark { display: inline-flex; align-items: center; gap: var(--s-2); font-size: var(--text-label); font-weight: var(--strong-weight); white-space: nowrap; color: var(--c); }
  .running { --c: var(--accent); } .waiting { --c: var(--warn); } .failed { --c: var(--danger); }
</style>
