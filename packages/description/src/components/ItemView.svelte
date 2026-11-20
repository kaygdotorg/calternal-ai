<script lang="ts">
  // The `item-view` host component (docs/description-contract.md 5.1 and 5.2): the retained or live body of the item the config names, read with `item_view.open` (the newest page first) and kept current through
  // `item_view.follow`: a `changed` signal fetches the newest head, an `invalidated` one clears what is drawn, and a refused follow is dropped and the view is opened again. A stale result is marked as belonging to an
  // earlier revision; an absent one draws a loading shell, never invented content. It follows the end while the person reads the end and never pulls a person who scrolled away; moving to another item cancels the old load.
  import { tick } from "svelte";
  import type { ItemViewConfig, ItemViewReply, ItemViewSignal } from "../draft";
  import { useApp, useActions } from "../invoke/context";
  import { resolveRef } from "./ref";
  import Snapshot from "./Snapshot.svelte";

  let { config, id }: { config: ItemViewConfig; id: string } = $props();
  const app = useApp(), { session } = useActions();
  const item = $derived(resolveRef(config.item, app));
  let reply = $state<ItemViewReply | null>(null), failed = $state(false), el = $state<HTMLElement>();
  let atEnd = true, generation = 0;

  async function open(it: string, ctl: AbortController): Promise<ItemViewReply | null> {
    try { return await session.exchange<ItemViewReply>("item_view.open", { type: "item_view", item: it, component: id, slot: config.slot, live: true }, ctl.signal); } catch { return null; }
  }
  $effect(() => {
    const it = item; const gen = ++generation; const ctl = new AbortController();
    reply = null; failed = false;
    if (!it) return;
    void (async () => {
      let r = await open(it, ctl);
      while (gen === generation && !ctl.signal.aborted) {
        if (!r) { failed = true; return; }
        reply = r; failed = false; await tick();
        if (atEnd && el) el.scrollTop = el.scrollHeight;
        if (!r.follow) return;
        let sig: ItemViewSignal | null = null;
        try { sig = await session.exchange<ItemViewSignal>("item_view.follow", { type: "item_view_follow", follow: r.follow }, ctl.signal); } catch { sig = null; }
        if (gen !== generation) return;
        if (sig?.state === "invalidated") reply = null;
        if (sig?.state === "closed") return;
        r = await open(it, ctl); // changed, invalidated or a refused follow: read the newest head again
      }
    })();
    return () => { ctl.abort(); };
  });
  const onscroll = () => { if (el) atEnd = el.scrollHeight - el.scrollTop - el.clientHeight < 40; };
</script>

<section class="iv" data-host-component="item-view" data-item={item} data-state={failed ? "error" : reply?.state ?? (item ? "loading" : "idle")} aria-label="Conversation">
  {#if !item}<p class="msg" data-view-state="idle">Select a thread, or start one.</p>
  {:else if failed}<p class="msg bad" role="alert" data-view-state="error">This thread could not be opened. It will be tried again when it changes.</p>
  {:else if reply?.state === "unavailable"}<p class="msg" data-view-state="unavailable">This thread is not available to you.</p>
  {:else if !reply || reply.state === "loading" || !reply.view}<p class="msg" role="status" data-view-state="loading">Opening the thread…</p>
  {:else}
    {#if reply.state === "stale"}<p class="stale" data-view-state="stale">Showing an earlier version. It will update when the thread wakes.</p>{/if}
    <div class="scroll" bind:this={el} {onscroll} tabindex="0" role="log" aria-live="polite" aria-label="Messages"><Snapshot node={reply.view.content} /></div>
  {/if}
</section>

<style>
  .iv { display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .scroll { overflow-y: auto; flex: 1; min-height: 0; padding: var(--s-4) var(--s-5); display: flex; flex-direction: column; gap: var(--s-2); }
  .scroll > :global(.s) { gap: var(--s-2); }
  .msg { margin: auto; color: var(--ink-3); text-align: center; padding: var(--s-6); } .msg.bad { color: var(--danger); }
  .stale { margin: 0; padding: var(--s-2) var(--s-5); color: var(--warn); font-size: var(--text-label); }
</style>
