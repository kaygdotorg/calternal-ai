<script lang="ts">
  // The message field on its own glass. Enter sends, Shift+Enter is a new line; while a turn runs the same field steers it and the
  // send button becomes Stop. The provider control chooses who answers the next message: it changes nothing visible in the thread
  // (the handover is internal), and never creates a thread.
  import { Glass, IconButton, Menu, Pill, Sheet, adaptive, currentForm, type MenuItem } from "@calternal/ui";
  import RunsOnPicker from "./RunsOnPicker.svelte";
  import { shell } from "../../shell/shell.svelte";
  import type { Permission, ProviderOption, RunsOn, ThreadSummary } from "./model";
  import { PERMISSIONS, permissionLabel, runsOnLabel } from "./permissions";

  let {
    thread,
    providers,
    machines,
    onsend,
    onstop,
    onprovider,
    onpermission,
    onrunson,
  }: {
    thread: ThreadSummary;
    providers: ProviderOption[];
    machines: string[];
    onsend: (text: string) => void;
    onstop: () => void;
    onprovider: (p: ThreadSummary["provider"], model: string) => void;
    onpermission: (p: Permission) => void;
    onrunson: (r: RunsOn) => void;
  } = $props();

  const drafts = new Map<string, string>();
  let text = $state("");
  let field = $state<HTMLTextAreaElement>();
  let menuOpen = $state(false);
  let permOpen = $state(false);
  let whereOpen = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let permTrigger = $state<HTMLButtonElement>();
  let whereTrigger = $state<HTMLButtonElement>();
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

  const permItems = $derived<MenuItem[]>(PERMISSIONS.map((p) => ({ id: p.id, label: p.label, sub: p.sub, icon: p.icon, checked: p.id === thread.permission })));
  const sheet = $derived(shell.narrow || currentForm() === "touch");
  const PERM_ICON = $derived(PERMISSIONS.find((p) => p.id === thread.permission)!.icon);

  $effect(() => {
    if (!whereOpen || sheet) return;
    const away = (e: PointerEvent) => {
      if (!(e.target as Element).closest?.(".where")) whereOpen = false;
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && ((whereOpen = false), whereTrigger?.focus());
    addEventListener("pointerdown", away, true);
    addEventListener("keydown", esc);
    return () => (removeEventListener("pointerdown", away, true), removeEventListener("keydown", esc));
  });

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
    <div class="chips">
      <span class="chip">
        <Pill label="{provider?.name ?? ''} · {modelName}" icon="spark" showLabel glass={false} aria-haspopup="menu" aria-expanded={menuOpen} bind:el={trigger} onclick={() => (menuOpen = !menuOpen)}>
          <span class="long">{provider?.name} · {modelName}</span><span class="mid">{modelName}</span>
        </Pill>
        <Menu open={menuOpen} {items} label="Provider and model" trigger={trigger} onclose={() => (menuOpen = false)} onpick={(id) => { const [p, m] = id.split(":") as [ThreadSummary["provider"], string]; onprovider(p, m); }} />
      </span>
      <span class="chip">
        <Pill label={permissionLabel(thread.permission)} icon={PERM_ICON} showLabel glass={false} aria-haspopup="menu" aria-expanded={permOpen} bind:el={permTrigger} onclick={() => (permOpen = !permOpen)}>
          <span class="long">{permissionLabel(thread.permission)}</span><span class="mid">{permissionLabel(thread.permission).split(" ")[0]}</span>
        </Pill>
        <Menu open={permOpen} items={permItems} label="Permission mode" trigger={permTrigger} onclose={() => (permOpen = false)} onpick={(id) => onpermission(id as Permission)} />
      </span>
      <span class="chip where">
        <Pill label={runsOnLabel(thread.runsOn)} icon="terminal" showLabel glass={false} aria-haspopup="dialog" aria-expanded={whereOpen} bind:el={whereTrigger} onclick={() => (whereOpen = !whereOpen)}>
          <span class="mach">{thread.runsOn.machine}</span><span class="long">· {thread.runsOn.mode === "sandboxed" ? "Sandboxed" : "Direct"}</span>
        </Pill>
        {#if whereOpen && !sheet}
          <div class="pop" role="dialog" aria-label="Where it runs" use:adaptive><Glass dense /><RunsOnPicker {machines} value={thread.runsOn} onchange={onrunson} /></div>
        {/if}
      </span>
    </div>
    {#if running && !text.trim()}
      <IconButton icon="stop" label="Stop the turn" tone="tint" onclick={onstop} />
    {:else}
      <IconButton icon="send" label={running ? "Steer the turn" : "Send message"} tone="accent" disabled={!text.trim()} onclick={send} />
    {/if}
  </div>
</div>

{#if sheet}
  <Sheet open={whereOpen} label="Where it runs" onclose={() => (whereOpen = false)}><RunsOnPicker {machines} value={thread.runsOn} onchange={onrunson} /></Sheet>
{/if}

<style>
  .composer { container: cmp / inline-size; position: relative; isolation: isolate; border-radius: var(--r-pane); padding: var(--s-4) var(--s-4) var(--s-4) var(--s-6); display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-2); }
  textarea { width: 100%; min-height: var(--pill-h); max-height: 9.5rem; padding: var(--s-3) 0; border: 0; background: none; resize: none; font-size: var(--text-ui); line-height: 1.5; color: var(--ink); overflow-y: auto; scrollbar-width: none; }
  textarea::placeholder { color: var(--ink-3); }
  textarea:focus-visible { outline: none; }
  .composer:has(textarea:focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tools { min-width: 0; display: flex; align-items: flex-end; justify-content: space-between; gap: var(--s-4); }
  /* One row always. Labels shorten as the composer narrows (container queries on the composer's own width): full, then the short form, then
     icons only (the full name stays as each chip's accessible name and tooltip). */
  .chips { flex: 1 1 0; display: flex; flex-wrap: nowrap; gap: var(--s-2); min-width: 0; }
  .chip { position: relative; flex: 0 1 auto; min-width: var(--pill-h); }
  .chip :global(.pill) { max-width: 100%; overflow: hidden; }
  .long, .mid, .mach { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mid { display: none; }
  .mach { max-width: 14em; }
  .where { flex-shrink: 1; min-width: var(--pill-h); }
  @container cmp (max-width: 680px) { .long { display: none; } .mid { display: inline; } .where .long { display: inline; } .mach { max-width: 6em; } }
  @container cmp (max-width: 480px) { .long, .mid, .mach { display: none; } .chip :global(.pill) { padding: 0; } }
  .pop { position: absolute; z-index: 30; bottom: calc(100% + var(--s-3)); left: 0; min-width: 18rem; isolation: isolate; border-radius: var(--r-card); animation: pop var(--dur-base) var(--ease-out); transform-origin: bottom left; }
  @keyframes pop { from { opacity: 0; scale: var(--enter-scale); } }
</style>
