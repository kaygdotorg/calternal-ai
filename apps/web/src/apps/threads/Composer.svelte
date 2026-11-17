<script lang="ts">
  // The message field on its own glass. Enter sends, Shift+Enter is a new line; while a turn runs the same field steers it and the
  // send button becomes Stop. The provider control chooses who answers the next message: it changes nothing visible in the thread
  // (the handover is internal), and never creates a thread.
  import { Glass, Icon, IconButton, Menu, Pill, adaptive, type MenuItem } from "@calternal/ui";
  import type { ProviderOption, ThreadSummary } from "./model";

  let {
    thread,
    providers,
    onsend,
    onstop,
    onprovider,
  }: { thread: ThreadSummary; providers: ProviderOption[]; onsend: (text: string) => void; onstop: () => void; onprovider: (p: ThreadSummary["provider"], model: string) => void } = $props();

  const drafts = new Map<string, string>();
  let text = $state("");
  let field = $state<HTMLTextAreaElement>();
  let menuOpen = $state(false);
  let trigger = $state<HTMLButtonElement>();
  $effect(() => {
    // A draft stays with its thread until sent.
    const id = thread.id;
    text = drafts.get(id) ?? "";
    return () => drafts.set(id, text);
  });
  const running = $derived(thread.state === "running");
  const provider = $derived(providers.find((p) => p.id === thread.provider));
  const modelName = $derived(provider?.models.find((m) => m.id === thread.model)?.name ?? thread.model);
  const items = $derived<MenuItem[]>(providers.flatMap((p) => p.models.map((m) => ({ id: `${p.id}:${m.id}`, label: m.name, sub: p.name, checked: p.id === thread.provider && m.id === thread.model }))));

  function grow() {
    if (!field) return;
    field.style.height = "auto";
    field.style.height = `${field.scrollHeight}px`;
  }
  function send() {
    const v = text.trim();
    if (!v) return;
    onsend(v);
    text = "";
    drafts.delete(thread.id);
    queueMicrotask(grow);
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    }
  }
</script>

<div class="composer" use:adaptive>
  <Glass />
  <label class="visually-hidden" for="composer-field">Message</label>
  <textarea id="composer-field" bind:this={field} bind:value={text} rows="1" oninput={grow} onkeydown={key} placeholder={running ? "Steer the running turn…" : `Message ${provider?.name ?? "the agent"}…`}></textarea>
  <div class="tools">
    <span class="prov">
      <Pill label="{provider?.name ?? ''} · {modelName}" icon="spark" showLabel glass={false} aria-haspopup="menu" aria-expanded={menuOpen} bind:el={trigger} onclick={() => (menuOpen = !menuOpen)} />
      <Menu open={menuOpen} {items} label="Provider and model" trigger={trigger} onclose={() => (menuOpen = false)} onpick={(id) => { const [p, m] = id.split(":") as [ThreadSummary["provider"], string]; onprovider(p, m); }} />
    </span>
    {#if running && !text.trim()}
      <IconButton icon="stop" label="Stop the turn" tone="tint" onclick={onstop} />
    {:else}
      <IconButton icon="send" label={running ? "Steer the turn" : "Send message"} tone="accent" disabled={!text.trim()} onclick={send} />
    {/if}
  </div>
</div>

<style>
  .composer { position: relative; isolation: isolate; border-radius: var(--r-panel); padding: var(--s-4) var(--s-4) var(--s-4) var(--s-6); display: grid; gap: var(--s-2); }
  textarea { width: 100%; min-height: var(--s-9); max-height: 9.5rem; padding: var(--s-3) 0; border: 0; background: none; resize: none; font-size: var(--text-body); line-height: 1.5; color: var(--ink); overflow-y: auto; scrollbar-width: none; }
  textarea::placeholder { color: var(--ink-3); }
  textarea:focus-visible { outline: none; }
  .composer:has(textarea:focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tools { display: flex; align-items: center; justify-content: space-between; gap: var(--s-4); }
  .prov { position: relative; min-width: 0; }
  .prov :global(.pill) { max-width: 100%; }
</style>
