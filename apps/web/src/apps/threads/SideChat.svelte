<script lang="ts">
  // The right sidebar's side chat: a short conversation about the open thread, separate from it. "Ask in Side Chat" on selected text opens this
  // panel with the selection quoted in the field. Fixture replies stand in for the provider; the real one answers through the same provider and
  // sandbox rules as the thread.
  import { IconButton, InlineEditor } from "@calternal/ui";
  import Md from "../../lib/Md.svelte";
  import { bridge } from "./compose.svelte";
  let { id }: { id?: string } = $props();
  type Msg = { role: "user" | "agent"; text: string };
  let messages = $state<Msg[]>([]);
  let draft = $state("");
  let field = $state<HTMLInputElement | HTMLTextAreaElement>();
  let handled = 0;
  $effect(() => {
    const q = bridge.side;
    if (!q || q.n === handled) return;
    handled = q.n;
    draft = q.quote;
    queueMicrotask(() => {
      field?.focus();
      field?.setSelectionRange(draft.length, draft.length);
    });
  });
  function send() {
    const t = draft.trim();
    if (!t) return;
    messages = [...messages, { role: "user", text: t }];
    draft = "";
    setTimeout(() => (messages = [...messages, { role: "agent", text: "This is a fixture reply. The side chat answers questions about the open thread without adding to it." }]), 700);
  }
</script>

<div class="side">
  <div class="log" role="log" aria-label="Side chat" aria-live="polite">
    {#if !messages.length}<p class="empty">Select text in the thread and choose “Ask in Side Chat”, or type a question about {id ? "this thread" : "a thread"}.</p>{/if}
    {#each messages as m}<div class="m {m.role} selectable"><Md text={m.text} /></div>{/each}
  </div>
  <div class="in">
    <InlineEditor id="side-field" class="side-input" multiline label="Ask in side chat" bind:el={field} bind:value={draft} onsubmit={() => send()} placeholder="Ask about this thread…" />
    <IconButton icon="send" label="Send to side chat" tone="accent" disabled={!draft.trim()} onclick={send} />
  </div>
</div>

<style>
  .side { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .log { flex: 1; min-height: 0; overflow-y: auto; display: grid; align-content: start; gap: var(--s-4); padding: var(--s-2) var(--s-3); }
  .empty { color: var(--ink-3); font-size: var(--text-label); }
  .m.user { justify-self: end; max-width: 90%; padding: var(--s-3) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); }
  .in { flex: none; display: flex; align-items: flex-end; gap: var(--s-2); padding: var(--s-3); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.06); }
  :global(.side-input) { flex: 1; padding: var(--s-3); font-size: var(--text-ui); min-height: max(calc(1lh + 2 * var(--s-3)), var(--pill-h)); max-height: 9.5rem; }
</style>
