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
  const label = (d: string, l?: string) => d === "deny" ? "Decline" : l === "this_item" ? "Always allow in this chat" : "Allow once";
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
  <!-- The decisions gather away and the outcome rises in place: height and opacity over --dur-base on --ease-out (no motion under reduced motion). -->
  <div class="fold" class:shut={shown !== "open"} aria-hidden={shown !== "open" || undefined}><div class="in">
    {#if left !== null}<span class="cd" data-countdown>{left > 0 ? `Without an answer in ${left} s your messages stay out of it.` : "Time is up: your messages stay out of it."}</span>{/if}
    <div class="b">{#each card.actions as a (a.decision)}{#if a.decision === "approve"}{#each a.lifetimes ?? [] as l (l)}<button type="button" class="p" disabled={busy || shown !== "open"} tabindex={shown === "open" ? 0 : -1} onclick={() => decide("approve", l)}>{label("approve", l)}</button>{/each}{:else}<button type="button" disabled={busy || shown !== "open"} tabindex={shown === "open" ? 0 : -1} onclick={() => decide("deny")}>{label("deny")}</button>{/if}{/each}</div>
  </div></div>
  {#if shown !== "open"}<span class="d res" data-request-result>{shown === "approved" ? "You allowed it." : shown === "denied" ? "You declined." : "No answer in time: your messages stayed out of it."}</span>{/if}
  {#if message}<span class="er" role="alert">{message}</span>{/if}
</aside>

<style>
  .rc { display: grid; gap: var(--s-2); padding: var(--s-4); border-radius: var(--r-card); background: var(--accent-soft); max-width: 30rem; font-size: var(--text-ui); }
  .d, .cd { color: var(--ink-2); font-size: var(--text-label); } .b { display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .b button { min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font-weight: var(--strong-weight); } .b .p { background: var(--accent-base); color: #fff; }
  .fold { display: grid; grid-template-rows: 1fr; transition: grid-template-rows var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out); } .fold.shut { grid-template-rows: 0fr; opacity: 0; } .fold > .in { overflow: clip; display: grid; gap: var(--s-2); min-height: 0; }
  .res { animation: rise var(--dur-base) var(--ease-out); } @keyframes rise { from { opacity: 0; translate: 0 var(--s-2); } to { opacity: 1; translate: 0 0; } }
  .er { color: var(--danger); }
</style>
