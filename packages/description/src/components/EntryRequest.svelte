<script lang="ts">
  // A host request card drawn under the entry it concerns (14.43 `requests`): a consent request for the person whose messages were selected for an agent. Only the designated person receives it. The decisions and
  // lifetimes are exactly the ones the host offered; they go to the same `request.resolve` route as any request card, with the card's own context. The countdown follows the host's `expires_at`; the host owns the timeout.
  import { onMount } from "svelte";
  import type { ActionOutcome } from "../model";
  import type { RequestCardRecord } from "../draft";
  import { useActions } from "../invoke/context";
  let { card }: { card: RequestCardRecord } = $props();
  const { session } = useActions();
  let result = $state<RequestCardRecord["state"] | null>(null), busy = $state(false), message = $state<string | null>(null), left = $state<number | null>(null);
  const shown = $derived(result ?? card.state);
  onMount(() => {
    if (!card.expires_at) return;
    const tick = () => { left = Math.max(0, Math.ceil((new Date(card.expires_at!).getTime() - Date.now()) / 1000)); }; tick();
    const h = setInterval(tick, 1000); return () => clearInterval(h);
  });
  const label = (d: string, l?: string) => d === "deny" ? "Decline" : l === "this_item" ? "Always allow in this thread" : "Allow once";
  async function decide(decision: "approve" | "deny", lifetime?: string) {
    busy = true; message = null;
    try {
      const o = await session.exchange<ActionOutcome>("request.resolve", { type: "request_resolve", card_context: card.card_context, request_ref: card.request_ref, decision, ...(lifetime ? { lifetime } : {}) });
      if (o.state === "ok") result = decision === "approve" ? "approved" : "denied"; else message = o.message ?? "That did not go through.";
    } catch { message = "The host did not answer."; } finally { busy = false; }
  }
</script>

<aside class="rc" data-entry-request data-state={shown} aria-label={card.title}>
  <strong>{card.title}</strong>{#if card.detail}<span class="d">{card.detail}</span>{/if}
  {#if shown === "open"}
    {#if left !== null}<span class="cd" data-countdown>{left > 0 ? `Without an answer in ${left} s your messages stay out of it.` : "Time is up: your messages stay out of it."}</span>{/if}
    <div class="b">{#each card.actions as a (a.decision)}{#if a.decision === "approve"}{#each a.lifetimes ?? [] as l (l)}<button type="button" class="p" disabled={busy} onclick={() => decide("approve", l)}>{label("approve", l)}</button>{/each}{:else}<button type="button" disabled={busy} onclick={() => decide("deny")}>{label("deny")}</button>{/if}{/each}</div>
  {:else}<span class="d" data-request-result>{shown === "approved" ? "You allowed it." : shown === "denied" ? "You declined." : "No answer in time: your messages stayed out of it."}</span>{/if}
  {#if message}<span class="er" role="alert">{message}</span>{/if}
</aside>

<style>
  .rc { display: grid; gap: var(--s-2); padding: var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--warn) 12%, transparent); max-width: 30rem; }
  .d, .cd { color: var(--ink-2); font-size: var(--text-label); } .b { display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .b button { min-height: 44px; padding: 0 var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 10%, transparent); color: var(--ink); } .b .p { background: var(--accent); color: var(--on-accent, #fff); }
  .er { color: var(--danger); }
</style>
