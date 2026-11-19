<script lang="ts">
  // The request-card host component: one protected record and its resolver. The card shows what the host sends as the trusted preview (title and the arguments, in the order the host wrote them) and ONLY the host's own
  // buttons resolve it: Approve (once, or for this thread) and Deny go to the `request_resolve` route with the host-issued card context and request ref. Nothing here is a plugin's: a description cannot relabel,
  // reorder or cover it. A request the host no longer has (resolved elsewhere) is shown as settled.
  import type { ActionOutcome, RequestCard } from "../model";
  import { useApp, useActions } from "../invoke/context";
  import { resolveRef } from "./ref";

  let { config, id }: { config: RequestCard["config"]; id: string } = $props();
  const app = useApp(), { session } = useActions();
  const request = $derived(resolveRef(config.request, app));
  interface Card { title: string; material: { label: string; value: unknown }[]; card_context?: string }
  let card = $state<Card | null>(null), outcome = $state<ActionOutcome | null>(null), busy = $state(false);
  $effect(() => {
    const r = request; card = null; outcome = null;
    if (!r) return;
    void session.exchange<Card>("request.card", { request_ref: r }).then((c) => { card = c; }, () => { card = null; });
  });
  async function decide(decision: "approve" | "deny", lifetime?: "once" | "this_thread") {
    if (!request || busy) return;
    busy = true;
    try {
      outcome = await session.exchange<ActionOutcome>("request.resolve", { type: "request_resolve", card_context: card?.card_context ?? session.viewContexts[id], request_ref: request, decision, ...(decision === "approve" ? { lifetime: lifetime ?? "once" } : {}) });
    } catch { outcome = { activation_id: "", state: "unknown", code: "unreachable", message: "The host did not answer, so it is not known whether this was resolved." }; }
    finally { busy = false; }
  }
</script>

{#if request && card}
  <section class="rc" data-host-component="request-card" data-host-component-id={id} aria-label={card.title}>
    <h3 class="t">{card.title}</h3>
    <dl class="m" data-material>{#each card.material as m (m.label)}<dt>{m.label}</dt><dd>{typeof m.value === "string" ? m.value : JSON.stringify(m.value)}</dd>{/each}</dl>
    {#if outcome && outcome.state !== "ok"}<div class="err" role="alert">{outcome.message}</div>
    {:else if outcome}<p class="done" role="status">Done.</p>
    {:else if !config.compact || true}<div class="b"><button type="button" aria-disabled={busy || undefined} onclick={() => decide("approve", "once")}>Approve once</button><button type="button" aria-disabled={busy || undefined} onclick={() => decide("approve", "this_thread")}>Approve for this thread</button><button type="button" aria-disabled={busy || undefined} onclick={() => decide("deny")}>Deny</button></div>{/if}
  </section>
{:else}<span data-host-component="request-card" data-host-component-id={id} hidden></span>{/if}

<style>
  .rc { display: grid; gap: var(--s-3); padding: var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--warn) 12%, transparent); }
  .t { margin: 0; font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .m { display: grid; grid-template-columns: auto 1fr; gap: var(--s-1) var(--s-4); margin: 0; } .m dt { color: var(--ink-3); } .m dd { margin: 0; overflow-wrap: anywhere; }
  .b { display: flex; flex-wrap: wrap; gap: var(--s-2); } .b button { min-height: 44px; padding: 0 var(--s-4); border-radius: var(--r-pill); background: color-mix(in oklab, var(--ink) 10%, transparent); color: var(--ink); }
  .err { color: var(--danger); } .done { margin: 0; color: var(--good); }
</style>
