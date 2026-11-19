<script lang="ts">
  // The turn-transcript host component (docs/description-contract.md 5.1): the conversation of the thread the config names, read through the host's `transcript.page` route (a checked, bounded page of committed events,
  // the screenful at the end first, older pages in steps), appended as the host commits more. It follows the end while the person reads the end and never pulls a person who scrolled away. A cursor the host refuses
  // (expired or revoked) is dropped and the thread is read again from the end; moving to another thread cancels the old load. When the first screenful is drawn it stamps `data-thread-rendered=<thread>` on the root,
  // dispatches `calternal:thread-rendered` and records the `calternal:thread:open` measure. Rich event kinds (code, pictures, tool steps) are the host's to draw: an event kind this renderer has no drawing for shows its
  // kind and its text, never nothing.
  import { tick } from "svelte";
  import type { TranscriptPage, TurnTranscript } from "../model";
  import { useApp, useActions } from "../invoke/context";
  import { resolveRef } from "./ref";

  let { config, id }: { config: TurnTranscript["config"]; id: string } = $props();
  const app = useApp(), { session } = useActions();
  const thread = $derived(resolveRef(config.thread, app));
  type Ev = TranscriptPage["events"][number];
  let events = $state<Ev[]>([]), cursor: string | undefined, loading = $state(false), failed = $state(false), el = $state<HTMLElement>();
  let atEnd = true, stamped = "", generation = 0;
  const follow = $derived((config.follow ?? "end") === "end");
  const limit = $derived(Math.min(128, config.window ?? 60));

  async function read(after: string | undefined, gen: number, ctl: AbortController): Promise<TranscriptPage | null> {
    try { return await session.exchange<TranscriptPage>("transcript.page", { thread, ...(after ? { after } : {}), limit }, ctl.signal); }
    catch { return null; }
  }

  $effect(() => {
    const t = thread; const gen = ++generation; const ctl = new AbortController();
    events = []; cursor = undefined; failed = false; stamped = "";
    if (!t) return;
    performance.mark("calternal:thread:start");
    loading = true;
    void (async () => {
      const first = await read(undefined, gen, ctl);
      if (gen !== generation) return;
      loading = false;
      if (!first) { failed = true; return; }
      events = first.events; cursor = first.committed_cursor;
      await tick();
      if (follow && el) el.scrollTop = el.scrollHeight;
      if (stamped !== t) {
        stamped = t;
        performance.measure("calternal:thread:open", "calternal:thread:start");
        document.documentElement.dataset.threadRendered = t;
        window.dispatchEvent(new CustomEvent("calternal:thread-rendered", { detail: { id: t, ms: performance.now() } }));
      }
      // Newly committed events arrive in steps after the cursor (the host's streaming is its own; this is the committed read).
      while (gen === generation && !ctl.signal.aborted) {
        await new Promise((r) => setTimeout(r, 1500));
        if (gen !== generation) return;
        const next = await read(cursor, gen, ctl);
        if (gen !== generation) return;
        if (!next) { cursor = undefined; const again = await read(undefined, gen, ctl); if (again && gen === generation) { events = again.events; cursor = again.committed_cursor; } continue; } // refused cursor: read again from the end
        if (next.events.length) { events = [...events, ...next.events]; cursor = next.committed_cursor; await tick(); if (follow && atEnd && el) el.scrollTop = el.scrollHeight; }
      }
    })();
    return () => { ctl.abort(); };
  });

  const onscroll = () => { if (el) atEnd = el.scrollHeight - el.scrollTop - el.clientHeight < 40; };
  const text = (e: Ev): string => { const d = e.data as Record<string, unknown>; return typeof d.text === "string" ? d.text : typeof d.summary === "string" ? d.summary : ""; };
  const who = (e: Ev): string => { const d = e.data as Record<string, unknown>; return typeof d.role === "string" ? d.role : e.kind; };
</script>

<section class="tr" data-host-component="turn-transcript" data-host-component-id={id} aria-label="Conversation" aria-busy={loading}>
  {#if !thread}<p class="note">Choose a thread.</p>
  {:else if failed}<p class="note bad" role="alert">The conversation could not be read.</p>
  {:else}
    <div class="scroll" bind:this={el} {onscroll} role="log" aria-live="polite" tabindex="0">
      {#each events as e (e.event_id)}<article class="ev {who(e)}" data-event={e.kind}><span class="who">{who(e)}</span><p>{text(e) || e.kind}</p></article>{/each}
      {#if loading}<p class="note">Loading…</p>{/if}
    </div>
  {/if}
</section>

<style>
  .tr { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .scroll { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: var(--s-3); padding: var(--s-3); }
  .ev { padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 6%, transparent); max-width: 44rem; }
  .ev.user { align-self: flex-end; background: var(--accent-soft); }
  .who { font-size: var(--text-label); color: var(--ink-3); } p { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  .note { color: var(--ink-3); padding: var(--s-4); } .bad { color: var(--danger); }
</style>
