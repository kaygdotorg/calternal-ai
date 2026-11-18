<script lang="ts">
  // Search inside threads. Results are messages, newest first, each with its thread, when it was written and the line that matched. The match is instant as you type
  // (the fixtures answer synchronously; a server answers per keystroke). Up and Down move through the results and the thread view behind follows (the address
  // changes by replacing, so Back does not collect every step); Enter or a tap opens the message and closes the search; Esc closes and goes back to where you were.
  // Nothing animates: the jump to a message is a plain scroll.
  import { untrack } from "svelte";
  import { Icon, InlineEditor, Sheet } from "@calternal/ui";
  import { navigate, route } from "../../lib/route.svelte";
  import { shell } from "../../shell/shell.svelte";
  import { client } from "./store.svelte";
  import { messageSearch } from "./search.svelte";
  import type { MessageHit } from "./model";
  let text = $state("");
  let sel = $state(0);
  let hits = $state<MessageHit[]>([]);
  let list = $state<HTMLElement>();
  let from = "";
  const available = typeof client.searchMessages === "function";
  const ago = (at: number) => new Date(at).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  $effect(() => {
    if (!messageSearch.open) return;
    text = ""; sel = 0; hits = [];
    // (read untracked: following a result changes the address, and this effect must not run again and clear the typed text)
    untrack(() => { const qs = route.query.toString(); from = route.path + (qs ? `?${qs}` : ""); });
    const f = () => document.querySelector<HTMLElement>(".msg-search-input");
    f()?.focus();
    requestAnimationFrame(() => { if (document.activeElement !== f()) f()?.focus(); });
  });
  let seq = 0;
  $effect(() => {
    const q = text.trim();
    if (!messageSearch.open || !q || !available) { hits = []; return; }
    const mine = ++seq;
    void client.searchMessages!(q, 100).then((r) => { if (mine === seq) { hits = r; sel = 0; } });
  });
  const target = (h: MessageHit) => `/threads/${h.threadId}?m=${encodeURIComponent(h.messageId)}`;
  function follow() { const h = hits[sel]; if (h) navigate(target(h), true); }
  function close(restore: boolean) { messageSearch.open = false; if (restore && from) navigate(from, true); }
  function open(h: MessageHit | undefined) { if (!h) return; navigate(target(h), true); close(false); }
  function key(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const n = hits.length;
    if (!n) return;
    sel = (sel + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
    follow();
    queueMicrotask(() => list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" }));
  }
</script>

<Sheet open={messageSearch.open} label="Search messages" onclose={() => close(true)} side={shell.narrow ? "bottom" : "top"} wide={!shell.narrow}>
  <div class="in">
    <div class="field">
      <Icon inline name="search" />
      <InlineEditor class="msg-search-input" label="Search messages in every thread" bind:value={text} placeholder="Search messages…" autoselect onsubmit={() => open(hits[sel])} onkeydown={key} />
    </div>
    {#if !available}
      <p class="none" role="status">Searching messages is not available on this server yet.</p>
    {:else if text.trim() && hits.length === 0}
      <p class="none" role="status">No message matches.</p>
    {/if}
    <ul bind:this={list} role="listbox" aria-label="Matching messages">
      {#each hits as h, i (h.threadId + h.messageId)}
        <li role="option" aria-selected={i === sel} class:on={i === sel} data-message={h.messageId} onpointermove={() => (sel = i)} onclick={() => open(h)} onkeydown={() => {}}>
          <span class="top"><span class="t">{h.threadTitle}</span><span class="w">{ago(h.at)}</span></span>
          <span class="line">{h.line.slice(0, h.start)}<mark>{h.line.slice(h.start, h.end)}</mark>{h.line.slice(h.end)}</span>
        </li>
      {/each}
    </ul>
  </div>
</Sheet>

<style>
  .in { display: grid; gap: var(--s-3); padding: var(--s-5); }
  .field { display: flex; align-items: center; gap: var(--s-4); min-height: var(--pill-h); padding: 0 var(--s-6); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-3); }
  :global(.msg-search-input) { position: relative; width: 100%; height: var(--pill-h); }
  .none { padding: 0 var(--s-4); font-size: var(--text-label); color: var(--ink-3); }
  ul { display: grid; gap: var(--s-1); max-height: min(60vh, 28rem); overflow-y: auto; scrollbar-width: none; }
  li { display: grid; gap: 2px; padding: var(--s-3) var(--s-5); border-radius: var(--r-select); cursor: pointer; min-width: 0; }
  li.on { background: var(--accent-soft); }
  .top { display: flex; gap: var(--s-4); justify-content: space-between; font-size: var(--text-label); color: var(--ink-3); min-width: 0; }
  .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: var(--strong-weight); color: var(--ink-2); }
  .w { flex: none; font-variant-numeric: tabular-nums; }
  .line { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-ui); }
  mark { background: color-mix(in oklab, var(--accent) 28%, transparent); color: inherit; border-radius: 2px; }
</style>
