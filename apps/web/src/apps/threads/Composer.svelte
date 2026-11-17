<script lang="ts">
  // The message field on its own glass. Enter sends, Shift+Enter is a new line; while a turn runs the same field steers it and the send button
  // becomes Stop. Under the field, one row of three chips: provider and model, permission mode, where it runs. They never wrap; as the composer
  // narrows their labels shorten in steps (container queries on the composer's own width) and stay meaningful:
  //   wide      "Claude Code · Sonnet 5.5"  "Approve for me"  "calternal-dev-02-std · Sandboxed"
  //   medium    "Sonnet 5.5"                "Approve for me"  machine + a mode icon
  //   narrow    provider icon only          "Approve for me"  machine + a mode icon
  //   phone     provider icon only          "Approve for me"  machine icon only
  // The permission mode's own name is the last label to go. Every chip keeps its full name as its accessible name and tooltip.
  // The provider control changes nothing visible in the thread (the handover is internal), and never creates a thread.
  import { Button, Icon, IconButton, Menu, Panel, Pill, Popover, Sheet, currentForm, type MenuItem } from "@calternal/ui";
  import RunsOnPicker from "./RunsOnPicker.svelte";
  import { bridge } from "./compose.svelte";
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
  // "Comment" on selected text: the quote, a blank line, then the caret.
  let handled = 0;
  $effect(() => {
    const q = bridge.composer;
    if (!q || q.n === handled) return;
    handled = q.n;
    text = (text.trim() ? text.replace(/\s+$/, "") + "\n\n" : "") + q.quote;
    queueMicrotask(() => {
      field?.focus();
      field?.setSelectionRange(text.length, text.length);
      grow();
    });
  });
  const running = $derived(thread.state === "running");
  const provider = $derived(providers.find((p) => p.id === thread.provider));
  const modelName = $derived(provider?.models.find((m) => m.id === thread.model)?.name ?? thread.model);
  const items = $derived<MenuItem[]>(providers.flatMap((p) => p.models.map((m) => ({ id: `${p.id}:${m.id}`, label: m.name, sub: p.name, checked: p.id === thread.provider && m.id === thread.model }))));
  const permItems = $derived<MenuItem[]>(PERMISSIONS.map((p) => ({ id: p.id, label: p.label, sub: p.sub, icon: p.icon, checked: p.id === thread.permission })));
  const sheet = $derived(shell.narrow || currentForm() === "touch");
  const PERM_ICON = $derived(PERMISSIONS.find((p) => p.id === thread.permission)!.icon);

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

<Panel class="composer" radius="pane">
  <div class="field">
    <label class="visually-hidden" for="composer-field">Message</label>
    <textarea id="composer-field" bind:this={field} bind:value={text} rows="1" oninput={grow} onkeydown={key} placeholder={running ? "Steer the running turn…" : `Message ${provider?.name ?? "the agent"}…`}></textarea>
    {#if running && !text.trim()}
      <IconButton icon="stop" label="Stop the turn" tone="tint" onclick={onstop} />
    {:else}
      <IconButton icon="send" label={running ? "Steer the turn" : "Send message"} tone="accent" disabled={!text.trim()} onclick={send} />
    {/if}
  </div>
  <div class="chips">
    <span class="chip prov">
      <Pill label="{provider?.name ?? ''} · {modelName}" icon="spark" showLabel glass={false} aria-haspopup="menu" aria-expanded={menuOpen} bind:el={trigger} onclick={() => (menuOpen = !menuOpen)}>
        <span class="pl">{provider?.name} · {modelName}</span><span class="pm">{modelName}</span>
      </Pill>
      <Menu open={menuOpen} {items} label="Provider and model" trigger={trigger} onclose={() => (menuOpen = false)} onpick={(id) => { const [p, m] = id.split(":") as [ThreadSummary["provider"], string]; onprovider(p, m); }} />
    </span>
    <span class="chip permc">
      <Pill label={permissionLabel(thread.permission)} icon={PERM_ICON} showLabel glass={false} aria-haspopup="menu" aria-expanded={permOpen} bind:el={permTrigger} onclick={() => (permOpen = !permOpen)}>
        <span class="perm">{permissionLabel(thread.permission)}</span>
      </Pill>
      <Menu open={permOpen} items={permItems} label="Permission mode" trigger={permTrigger} onclose={() => (permOpen = false)} onpick={(id) => onpermission(id as Permission)} />
    </span>
    <span class="chip where">
      <Pill label={runsOnLabel(thread.runsOn)} icon="terminal" showLabel glass={false} aria-haspopup="dialog" aria-expanded={whereOpen} bind:el={whereTrigger} onclick={() => (whereOpen = !whereOpen)}>
        <span class="rl">{thread.runsOn.machine} · {thread.runsOn.mode === "sandboxed" ? "Sandboxed" : "Direct"}</span><span class="rm">{thread.runsOn.machine}</span><span class="mode"><Icon inline name={thread.runsOn.mode === "sandboxed" ? "shield" : "folder"} /></span>
      </Pill>
      {#if !sheet}
        <Popover open={whereOpen} anchor={whereTrigger} label="Where it runs" trigger={whereTrigger} onclose={() => (whereOpen = false)}><RunsOnPicker {machines} value={thread.runsOn} onchange={onrunson} /></Popover>
      {/if}
    </span>
  </div>
</Panel>

{#if sheet}
  <Sheet open={whereOpen} label="Where it runs" onclose={() => (whereOpen = false)}><RunsOnPicker {machines} value={thread.runsOn} onchange={onrunson} /><div class="done"><Button variant="primary" text="Done" onclick={() => (whereOpen = false)} /></div></Sheet>
{/if}

<style>
  :global(.composer) { container: cmp / inline-size; padding: var(--s-3) var(--s-3) var(--s-3) var(--s-6); display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-1); }
  .field { display: flex; align-items: flex-end; gap: var(--s-3); }
  /* The caret, the placeholder and the typed text are one box: same font, same line height, same inset, so the caret sits exactly on the text's baseline.
     The height is a whole number of lines plus the vertical padding, never a fixed pixel height that leaves the text off centre. */
  textarea {
    flex: 1; min-width: 0; margin: 0; padding: var(--s-3) 0; border: 0; border-radius: 0; background: none; resize: none; overflow-y: auto; scrollbar-width: none;
    font: inherit; font-size: var(--text-ui); line-height: 1.5; letter-spacing: inherit; color: var(--ink); caret-color: var(--accent);
    min-height: calc(1lh + 2 * var(--s-3)); max-height: 9.5rem;
  }
  textarea::placeholder { color: var(--ink-3); opacity: 1; }
  textarea:focus, textarea:focus-visible { outline: none; }
  :global(:root[data-input="key"] .composer:has(textarea:focus-visible)) { outline: 2px solid var(--accent); outline-offset: 2px; }
  .chips { display: flex; flex-wrap: nowrap; gap: var(--s-2); min-width: 0; padding-right: var(--s-3); }
  .chip { position: relative; flex: 0 1 auto; min-width: var(--pill-h); }
  .chip :global(.pill) { max-width: 100%; overflow: hidden; }
  .pl, .pm, .perm, .rl, .rm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pm, .rm, .mode { display: none; }
  .where { flex-shrink: 1; }
  @container cmp (max-width: 700px) { .pl, .rl { display: none; } .pm, .rm { display: inline; } .mode { display: inline-grid; } }
  @container cmp (max-width: 500px) { .pm { display: none; } .prov :global(.pill) { padding: 0; } }
  @container cmp (max-width: 420px) { .rm, .mode { display: none; } .where :global(.pill) { padding: 0; } }
  @container cmp (max-width: 290px) { .perm { display: none; } .permc :global(.pill) { padding: 0; } }
  .done { display: flex; justify-content: flex-end; padding: 0 var(--s-5) var(--s-5); }
</style>
