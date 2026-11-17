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
  import { Button, Icon, IconButton, InlineEditor, Menu, Panel, Pill, Popover, Sheet, currentForm, type IconName, type MenuItem } from "@calternal/ui";
  import { marksOf } from "../../palette/engine";
  import { fixtureItems } from "../../palette/fixtures";
  import { mentionPool, pick as pickMentions, skillPool, triggerAt, type Mention } from "../../palette/mentions";
  import { store } from "./store.svelte";
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
  let field = $state<HTMLInputElement | HTMLTextAreaElement>();
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
    });
  });
  // ---- the same engine as the palette: highlighted words, @ for people, agents, threads and items, / for skills ----
  const marks = $derived(marksOf(text));
  const pool = $derived(mentionPool(providers.map((p) => p.name), store.threads.filter((t) => t.id !== thread.id).map((t) => ({ id: t.id, title: t.title })), fixtureItems().filter((i) => i.kind === "note")));
  let caret = $state(0);
  let dismissed = $state<number | null>(null); // the trigger position the person closed with Escape
  const trig = $derived(triggerAt(text, caret));
  const groups = $derived(trig && trig.start !== dismissed ? pickMentions(trig.kind === "skill" ? skillPool() : pool, trig.query) : []);
  const flat = $derived(groups.flatMap((g) => g.entries));
  let sel = $state(0);
  $effect(() => { flat; sel = 0; });
  let fieldBox = $state<HTMLElement>();
  const here = () => { caret = field?.selectionStart ?? text.length; };
  function choose(m: Mention) {
    if (!trig || !field) return;
    const end = caret;
    const tail = text.slice(end);
    const gap = m.kind === "skill" || /^\s/.test(tail) ? "" : " ";
    text = text.slice(0, trig.start) + m.insert + (m.kind === "skill" ? "" : gap) + tail;
    const at = trig.start + m.insert.length + (m.kind === "skill" ? 0 : gap.length);
    queueMicrotask(() => { field?.focus(); field?.setSelectionRange(at, at); caret = at; });
  }
  function onkey(e: KeyboardEvent) {
    if (!flat.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); sel = (sel + (e.key === "ArrowDown" ? 1 : flat.length - 1)) % flat.length; }
    else if (e.key === "Enter" || e.key === "Tab") { e.preventDefault(); choose(flat[sel]!); }
    else if (e.key === "Escape") { e.preventDefault(); dismissed = trig?.start ?? null; }
  }
  const KIND_ICON: Record<string, IconName> = { person: "user", agent: "spark", thread: "threads", item: "notes", skill: "wrench" };
  // What is open, as chips above the field: the workspace the thread works in, and each @mention in the text (removable).
  const mentions = $derived(marks.filter((m) => m.kind === "mention" || m.kind === "person").map((m) => ({ ...m, label: text.slice(m.start + 1, m.end).replace(/^\[|\]$/g, "") })));
  const dropMention = (m: { start: number; end: number }) => { text = (text.slice(0, m.start) + text.slice(m.end)).replace(/ {2,}/g, " ").replace(/^ /, ""); queueMicrotask(() => field?.focus()); };
  const running = $derived(thread.state === "running");
  const provider = $derived(providers.find((p) => p.id === thread.provider));
  const modelName = $derived(provider?.models.find((m) => m.id === thread.model)?.name ?? thread.model);
  const items = $derived<MenuItem[]>(providers.flatMap((p) => p.models.map((m) => ({ id: `${p.id}:${m.id}`, label: m.name, sub: p.name, icon: p.id === "codex" ? "codex" : "spark", checked: p.id === thread.provider && m.id === thread.model }))));
  const providerIcon = $derived(thread.provider === "codex" ? "codex" : "spark");
  const permItems = $derived<MenuItem[]>(PERMISSIONS.map((p) => ({ id: p.id, label: p.label, sub: p.sub, icon: p.icon, checked: p.id === thread.permission })));
  const sheet = $derived(shell.narrow || currentForm() === "touch");
  const PERM_ICON = $derived(PERMISSIONS.find((p) => p.id === thread.permission)!.icon);

  function send() {
    const v = text.trim();
    if (!v) return;
    onsend(v);
    text = "";
    drafts.delete(thread.id);
  }
</script>

<Panel class="composer" radius="pane">
  <div class="ctx" aria-label="Context">
    <span class="cx"><Icon inline name="folder" /><span>{thread.workspace}</span></span>
    {#each mentions as m}<span class="cx on"><Icon inline name={m.label === provider?.name || pool.find((p) => p.label === m.label)?.kind === "agent" ? "spark" : pool.find((p) => p.label === m.label)?.kind === "thread" ? "threads" : pool.find((p) => p.label === m.label)?.kind === "item" ? "notes" : "user"} /><span>{m.label}</span><button type="button" class="x" aria-label="Remove {m.label}" onclick={() => dropMention(m)}><Icon inline name="close" /></button></span>{/each}
  </div>
  <div class="field" bind:this={fieldBox}>
    <InlineEditor id="composer-field" class="field-input" multiline label="Message" bind:el={field} bind:value={text} {marks} onselect={here} oninput={here} onkeydown={onkey} onsubmit={() => send()} placeholder={running ? "Steer the running turn…" : `Message ${provider?.name ?? "the agent"}…`} />
    {#if running && !text.trim()}
      <IconButton icon="stop" label="Stop the turn" tone="tint" onclick={onstop} />
    {:else}
      <IconButton icon="send" label={running ? "Steer the turn" : "Send message"} tone="accent" disabled={!text.trim()} onclick={send} />
    {/if}
  </div>
  <div class="chips">
    <span class="chip prov">
      <Pill label="{provider?.name ?? ''} · {modelName}" icon={providerIcon} showLabel glass={false} aria-haspopup="menu" aria-expanded={menuOpen} bind:el={trigger} onclick={() => (menuOpen = !menuOpen)}>
        <span class="pl">{provider?.name} · {modelName}</span><span class="pm">{modelName}</span>
      </Pill>
      <Menu open={menuOpen} {items} search label="Provider and model" trigger={trigger} onclose={() => (menuOpen = false)} onpick={(id) => { const [p, m] = id.split(":") as [ThreadSummary["provider"], string]; onprovider(p, m); }} />
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

<Popover open={groups.length > 0} anchor={fieldBox} prefer="top" role="listbox" label={trig?.kind === "skill" ? "Skills" : "Mention"} onclose={() => (dismissed = trig?.start ?? null)}>
  <div class="mp">
    {#each groups as g (g.kind)}
      <div class="mg" role="group" aria-label={g.title}>
        <h3>{g.title}</h3>
        {#each g.entries as m (m.id)}
          {@const i = flat.indexOf(m)}
          <div class="mo" class:on={i === sel} role="option" tabindex="-1" aria-selected={i === sel} onpointermove={() => (sel = i)} onpointerdown={(e) => { e.preventDefault(); choose(m); }}><Icon inline name={KIND_ICON[m.kind]!} /><span class="l">{m.label}</span><span class="g">{m.sub}</span></div>
        {/each}
      </div>
    {/each}
  </div>
</Popover>

{#if sheet}
  <Sheet open={whereOpen} label="Where it runs" onclose={() => (whereOpen = false)}><RunsOnPicker {machines} value={thread.runsOn} onchange={onrunson} /><div class="done"><Button variant="primary" text="Done" onclick={() => (whereOpen = false)} /></div></Sheet>
{/if}

<style>
  :global(.composer) { container: cmp / inline-size; padding: var(--s-3) var(--s-3) var(--s-3) var(--s-6); display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-1); }
  .ctx { display: flex; flex-wrap: wrap; gap: var(--s-2); padding: 0 var(--s-3) 0 0; }
  .cx { display: inline-flex; align-items: center; gap: var(--s-2); max-width: 100%; padding: var(--s-1) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-2); font-size: var(--text-label); }
  .cx > span:not(:first-child) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cx.on { background: color-mix(in oklab, var(--accent) 16%, transparent); color: var(--ink); }
  .x { display: inline-grid; place-items: center; color: var(--ink-3); border-radius: 50%; }
  .mp { display: grid; gap: var(--s-2); min-width: 16rem; max-width: min(26rem, 90vw); padding: var(--s-2); }
  .mg h3 { padding: var(--s-2) var(--s-4); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .mo { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-4); border-radius: var(--r-select); cursor: pointer; font-size: var(--text-ui); }
  .mo.on { background: var(--accent-soft); }
  .mo .l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mo .g { font-size: var(--text-label); color: var(--ink-3); }
  .field { display: flex; align-items: flex-end; gap: var(--s-3); }
  /* The caret, the placeholder and the typed text are one box: same font, same line height, same inset, so the caret sits exactly on the text's baseline.
     The height is a whole number of lines plus the vertical padding, never a fixed pixel height that leaves the text off centre. */
  :global(.field-input) { flex: 1; padding: var(--s-3) 0; overflow-y: auto; scrollbar-width: none; font-size: var(--text-ui); min-height: max(calc(1lh + 2 * var(--s-3)), var(--pill-h)); max-height: 9.5rem; }
  :global(:root[data-input="key"] .composer:has(.field-input:focus-visible)) { outline: 2px solid var(--accent); outline-offset: 2px; }
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
