<script lang="ts">
  // The message field on its own glass. Enter sends, Shift+Enter is a new line; while a turn runs the same field steers it and the send button
  // becomes Stop. Under the field, one row of four chips: attach, model (with its reasoning level), permission mode, where it runs; the row gives up words by priority as the box narrows (PillRow). They never wrap; as the composer
  // narrows their labels shorten in steps (container queries on the composer's own width) and stay meaningful:
  //   wide      "Claude Code · Sonnet 5.5"  "Approve for me"  "calternal-dev-02-std · Sandboxed"
  //   medium    "Sonnet 5.5"                "Approve for me"  machine + a mode icon
  //   narrow    provider icon only          "Approve for me"  machine + a mode icon
  //   phone     provider icon only          "Approve for me"  machine icon only
  // The permission mode's own name is the last label to go. Every chip keeps its full name as its accessible name and tooltip.
  // The provider control changes nothing visible in the thread (the handover is internal), and never creates a thread.
  import { motionAllowed, snapshot, play, type FoldSnap } from "./fold";
  import { Button, Icon, InlineEditor, Menu, Panel, Pill, PillRow, Popover, Sheet, currentForm, type IconName, type MenuItem } from "@calternal/ui";
  import type { Attachment } from "./model";
  import type { FieldEditor, FieldTrigger } from "../../../../../packages/editor/src/index";
  import { marksOf } from "../../palette/engine";
  import { fixtureItems } from "../../palette/fixtures";
  import { mentionPool, pick as pickMentions, remoteMentionPool, remoteSkillPool, skillPool, warmPool, type Mention } from "../../palette/mentions";
  import { pickerKey } from "../../../../../packages/editor/src/text/picker";
  import { client, store } from "./store.svelte";
  import RunsOnPicker from "./RunsOnPicker.svelte";
  import ActiveChildren from "./ActiveChildren.svelte";
  import LevelMark from "./LevelMark.svelte";
  import SendControl from "./SendControl.svelte";
  import MentionPicker from "../../palette/MentionPicker.svelte";
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
    onreasoning,
    onattach,
    ontype,
  }: {
    thread: ThreadSummary;
    providers: ProviderOption[];
    machines: string[];
    onsend: (text: string, attachments: Attachment[]) => void | Promise<void>;
    onstop: () => void;
    onprovider: (p: ThreadSummary["provider"], model: string) => void;
    onpermission: (p: Permission) => void;
    onrunson: (r: RunsOn) => void;
    onreasoning: (level: string | null) => void;
    onattach: (file: File, progress: (fraction: number) => void) => Promise<void>;
    /** The person started typing: called once per focus of the box (the server may then offer a provider sign-in the thread will need). */
    ontype?: () => void;
  } = $props();
  let typed = false;
  // The box lost focus: the next typing is a new one (once per focus).
  $effect(() => { if (!focused) typed = false; });

  const drafts = new Map<string, string>();
  let text = $state("");
  let field = $state<HTMLInputElement | HTMLTextAreaElement>();
  let editor = $state<FieldEditor | undefined>();
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
  // Warm the recogniser once when the composer appears (its regular expressions compile on first use): the first key typed should cost what the others do.
  $effect(() => { marksOf("warm @a friday 5pm !high #t"); });
  // The first key must run warm code: at idle after mount the field is sent a key that does nothing (F24, then its key-up) and an input event with no change, so the shortcut lookup, the field editor's
  // key and input handlers, the undo grouping and the trigger rules have all run once before the person types. (Marked `warm`: the page's input modality ignores it.)
  $effect(() => {
    const el = field;
    if (!el) return;
    const id = (window.requestIdleCallback ?? ((f: () => void) => setTimeout(f, 300)))(() => {
      for (const type of ["keydown", "keyup"]) { const e = new KeyboardEvent(type, { key: "F24", bubbles: true, cancelable: true }); (e as KeyboardEvent & { warm?: boolean }).warm = true; el.dispatchEvent(e); }
      el.dispatchEvent(new InputEvent("input", { bubbles: true }));
    });
    return () => (window.cancelIdleCallback ?? clearTimeout)(id as number);
  });
  const marks = $derived(marksOf(text));
  // With a server the pickers are its lookups (people.lookup, threads.lookup, items.lookup for `@`; skills.list for `/`), asked once per pause in typing; without one they are the fixtures.
  type Found = { people: { id: string; name: string; kind: string }[]; threads: { id: string; title: string }[]; items: { id: string; title: string; kind: string }[]; skills: { name: string; description: string }[] };
  let found = $state<Found>({ people: [], threads: [], items: [], skills: [] });
  let lookupSeq = 0;
  $effect(() => {
    const L = client.lookups, t = trig;
    if (!L || !t) return;
    const kind = t.kind, query = t.query, seq = ++lookupSeq;
    const h = setTimeout(async () => {
      try {
        if (kind === "skill") { const skills = await L.skills(query); if (seq === lookupSeq) found = { ...found, skills }; return; }
        const [people, threads, items] = await Promise.all([L.people(query), query ? L.threads(query) : Promise.resolve([]), query ? L.items(query) : Promise.resolve([])]);
        if (seq === lookupSeq) found = { ...found, people, threads: threads.filter((x) => x.id !== thread.id), items };
      } catch { /* the picker keeps what it had: a failed lookup is not an error the person can act on while typing */ }
    }, 120);
    return () => clearTimeout(h);
  });
  const pool = $derived(client.lookups ? remoteMentionPool(providers.map((p) => p.name), found) : mentionPool(providers.map((p) => p.name), store.threads.filter((t) => t.id !== thread.id).map((t) => ({ id: t.id, title: t.title })), fixtureItems().filter((i) => i.kind === "note")));
  const skills = $derived(client.lookups ? remoteSkillPool(found.skills) : skillPool());
  // The pickers' search indexes are built when the composer is idle, not on the first `@` or `/`.
  $effect(() => { const p = pool, k = skills; const id = (window.requestIdleCallback ?? ((f: () => void) => setTimeout(f, 200)))(() => { warmPool(p); warmPool(k); }); return () => (window.cancelIdleCallback ?? clearTimeout)(id as number); });
  // The trigger at the caret (`@` or a line-start `/`) comes from the shared text layer: the field reports it when it changes (packages/editor/src/text/triggers.ts), so no caret is tracked here.
  let trig = $state<FieldTrigger | null>(null);
  let dismissed = $state<number | null>(null); // the trigger position the person closed with Escape
  const groups = $derived(trig && trig.from !== dismissed ? pickMentions(trig.kind === "skill" ? skills : pool, trig.query) : []);
  const flat = $derived(groups.flatMap((g) => g.entries));
  let sel = $state(0);
  $effect(() => { flat; sel = 0; });
  let fieldBox = $state<HTMLElement>();
  function choose(m: Mention) {
    const t = trig;
    if (!t || !field) return;
    const tail = text.slice(t.to);
    const gap = m.kind === "skill" || /^\s/.test(tail) ? "" : " ";
    const insert = m.insert + (m.kind === "skill" ? "" : gap);
    // One change through the field editor: a single undo step, the caret after it.
    if (editor) editor.apply({ from: t.from, to: t.to, insert });
    else text = text.slice(0, t.from) + insert + tail;
    const at = t.from + insert.length;
    trig = null;
    queueMicrotask(() => { field?.focus(); field?.setSelectionRange(at, at); });
  }
  // The picker's keys are the shared ones (Up and Down wrap, Enter and Tab choose, Escape closes).
  const onkey = (e: KeyboardEvent) => { if (!typed && e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) { typed = true; ontype?.(); } return pickerKey(e, { count: flat.length, index: sel, set: (i) => (sel = i), choose: () => choose(flat[sel]!), dismiss: () => (dismissed = trig?.from ?? null) }); };
  // What is open, as chips above the field: the workspace the thread works in, and each @mention in the text (removable).
  const mentions = $derived(marks.filter((m) => m.kind === "mention" || m.kind === "person").map((m) => ({ ...m, label: text.slice(m.start + 1, m.end).replace(/^\[|\]$/g, "") })));
  const dropMention = (m: { start: number; end: number }) => { text = (text.slice(0, m.start) + text.slice(m.end)).replace(/ {2,}/g, " ").replace(/^ /, ""); queueMicrotask(() => field?.focus()); };
  // Collapsed when not in use, like ChatGPT's and Claude's: the context and chips rows fold away (and on a phone the box takes a wider inset) until the composer has focus, or one of its menus is open.
  // Focus is read after the event (a window losing focus keeps the field active and so keeps the composer open). Pointer, touch and keyboard focus of the field or a chip set it; a mouse press on a button
  // (Stop, Send) does not (such a button is not `:focus-visible`): pressing it must not move it away from under the pointer by expanding the box before the click is done. The keyboard's change is instant.
  let panelEl = $state<HTMLElement>();
  let focused = $state(false);
  // A press inside the box keeps it open until the click is done: a button that takes the focus on press (or, in Safari, does not take it at all) must not have its row taken out from under the pointer.
  let pressing = $state(false);
  function press() {
    pressing = open; // only an open box is held open: a press on Stop or Send of a collapsed one must not unfold it under the pointer
    const done = () => { removeEventListener("pointerup", done, true); removeEventListener("pointercancel", done, true); setTimeout(() => (pressing = false), 0); };
    addEventListener("pointerup", done, true); addEventListener("pointercancel", done, true);
  }
  // A tap anywhere in the collapsed box that is not on a control focuses the field (ChatGPT's and bb's): the box is the field. A text field takes focus (and opens the keyboard) from the tap itself; `:focus-visible` is only the test for buttons.
  function tapBox(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button, a, [role=button], [contenteditable], textarea, input, [role=menu], [role=dialog]")) return;
    field?.focus();
  }
  const open = $derived(focused || pressing || menuOpen || permOpen || whereOpen || trig !== null);
  // The morph: the state above flips the layout at once; before the DOM changes the old look is read, after it the motion is played from it (fold.ts). Not for keyboard use or reduced motion, and the
  // list above is told (`cmp-fold`) so it keeps the last message above the composer and slides with it.
  let prevOpen: boolean | undefined;
  let snap: FoldSnap | null = null;
  $effect.pre(() => { const now = open; if (prevOpen !== undefined && now !== prevOpen && panelEl) snap = snapshot(panelEl, !now); });
  $effect(() => {
    const now = open;
    if (prevOpen === undefined) { prevOpen = now; return; }
    if (now === prevOpen || !panelEl) return;
    prevOpen = now;
    const go = snap; snap = null;
    const animate = !!go && motionAllowed();
    const cs = getComputedStyle(document.documentElement);
    // A custom property comes back as the browser normalised it (".35s"), so the unit is read.
    const dur = cs.getPropertyValue("--dur-morph").trim();
    const ms = (dur.endsWith("ms") ? parseFloat(dur) : parseFloat(dur) * 1000) || 350;
    const easing = cs.getPropertyValue("--ease-morph").trim() || "ease-out";
    panelEl.dispatchEvent(new CustomEvent("cmp-fold", { bubbles: true, detail: { animate, ms, easing } }));
    if (animate && go) play(panelEl, go, ms, easing);
  });
  const running = $derived(thread.state === "running");
  // A turn parked on a request of yours is still the person's to stop (stopping cancels the request; the server refuses a new message to it). An interrupted turn has its own notice.
  const parked = $derived(thread.state === "waiting" && !thread.notice);
  const provider = $derived(providers.find((p) => p.id === thread.provider));
  const modelName = $derived(provider?.models.find((m) => m.id === thread.model)?.name ?? thread.model);
  // The model menu: the models, then (for a model that accepts levels) its reasoning levels, in one list: the level is part of the model chip (docs/composer.md).
  const modelOpt = $derived(provider?.models.find((m) => m.id === thread.model));
  const levelsOf = $derived(modelOpt?.levels ?? []);
  const cap = (l: string) => l.charAt(0).toUpperCase() + l.slice(1);
  const items = $derived<MenuItem[]>([
    ...providers.flatMap((p) => p.models.map((m) => ({ id: `${p.id}:${m.id}`, label: m.name, sub: p.listed === false ? `${p.name} · models are not listed yet` : p.name, icon: (p.id === "codex" ? "codex" : "spark") as IconName, checked: p.id === thread.provider && m.id === thread.model }))),
    ...levelsOf.map((l) => ({ id: `level:${l}`, label: `${cap(l)} reasoning`, checked: thread.reasoning === l })),
  ]);
  function pickModel(id: string) {
    if (id.startsWith("level:")) return onreasoning(id.slice(6) || null);
    const [p, m] = id.split(":") as [ThreadSummary["provider"], string];
    onprovider(p, m);
    // A level the new model does not accept is dropped (the provider's own default then applies).
    const next = providers.find((x) => x.id === p)?.models.find((x) => x.id === m)?.levels ?? [];
    if (thread.reasoning && !next.includes(thread.reasoning)) onreasoning(null);
  }
  type Chip = "attach" | "model" | "perm" | "where";
  const CHIPS: Chip[] = ["attach", "model", "perm", "where"];
  /** The model chip's full name, for its tooltip and accessible name when it shows only its icon: a second fact is a comma away, never a dot. */
  const modelFull = $derived([provider?.name, modelName, thread.reasoning ? `${thread.reasoning} reasoning` : ""].filter(Boolean).join(", "));
  // A chip holding a choice the person made keeps its words longest (docs/composer.md, decision 6): a model other than the provider's first, a reasoning level, a permission other than Approve for me, a machine other than Automatic.
  const keepWords = $derived<Chip[]>([
    ...((thread.reasoning || (provider && !modelOpt?.default && thread.model !== "default")) ? ["model" as const] : []),
    ...(thread.permission !== "approve" ? ["perm" as const] : []),
    ...(thread.runsOn.machine !== "Automatic" ? ["where" as const] : []),
  ]);
  // A search that finds nothing says why when the cause is the missing list, not the words: the server has no list of this provider's models yet (it sends with its default).
  const empty = $derived(providers.some((p) => p.listed === false) ? "This provider's models are not listed yet. It uses its default model." : "Nothing matches.");
  const providerIcon = $derived(thread.provider === "codex" ? "codex" : "spark");
  // A mode picked here applies to the next turn at once, wider or narrower, with no confirmation (owner decision 2026-11-18); a turn that is running keeps the mode it started with, which the chip and menu say.
  const permItems = $derived<MenuItem[]>(PERMISSIONS.map((p) => ({ id: p.id, label: p.label, sub: p.sub, icon: p.icon, checked: p.id === thread.permission })));
  const sheet = $derived(shell.narrow || currentForm() === "touch");
  const PERM_ICON = $derived(PERMISSIONS.find((p) => p.id === thread.permission)!.icon);

  // ---- attachments: a sticky row above the text (see docs/threads-ui.md, "Attachments") ----
  // Pictures and files come from the + chip's menu (a file picker), from a paste and from a drop. Each is uploaded at once (`onattach`), shows its progress, and can be removed; the message is sent with the ones
  // that are ready, and sending waits for uploads still running. The row is its own part of the box: it stays when the composer folds, and the list above follows the box's height.
  let attachments = $state<Attachment[]>([]);
  let attachOpen = $state(false);
  let attachTrigger = $state<HTMLButtonElement>();
  let picker = $state<HTMLInputElement>();
  let dropping = $state(false);
  let seq = 0;
  const attachItems: MenuItem[] = [{ id: "photos", label: "Photos", icon: "image" }, { id: "files", label: "Files", icon: "file" }];
  function pickAttach(id: string) { attachOpen = false; if (picker) { picker.accept = id === "photos" ? "image/*" : ""; picker.click(); } }
  function addFiles(files: Iterable<File>) {
    for (const f of files) {
      const a: Attachment = { id: `a${++seq}`, kind: f.type.startsWith("image/") ? "image" : "file", name: f.name || "pasted file", size: f.size, url: f.type.startsWith("image/") ? URL.createObjectURL(f) : undefined, progress: 0, status: "uploading" };
      attachments = [...attachments, a];
      const set = (patch: Partial<Attachment>) => (attachments = attachments.map((x) => (x.id === a.id ? { ...x, ...patch } : x)));
      onattach(f, (p) => set({ progress: p })).then(() => set({ status: "ready", progress: 1 }), (e: Error) => set({ status: "failed", error: e.message }));
    }
    queueMicrotask(() => field?.focus());
  }
  function removeAttachment(id: string) {
    const a = attachments.find((x) => x.id === id);
    if (a?.url) URL.revokeObjectURL(a.url);
    attachments = attachments.filter((x) => x.id !== id);
  }
  const uploading = $derived(attachments.some((a) => a.status === "uploading"));
  const ready = $derived(attachments.filter((a) => a.status === "ready"));
  const canSend = $derived(!uploading && (!!text.trim() || ready.length > 0));
  const size = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : n >= 1024 ? `${Math.round(n / 1024)} KB` : `${n} B`);
  const filesOf = (dt: DataTransfer | null) => [...(dt?.files ?? [])];
  function onpaste(e: ClipboardEvent) { const f = filesOf(e.clipboardData); if (f.length) { e.preventDefault(); addFiles(f); } }
  function ondrop(e: DragEvent) { dropping = false; const f = filesOf(e.dataTransfer); if (f.length) { e.preventDefault(); addFiles(f); } }
  function ondragover(e: DragEvent) { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); dropping = true; } }

  function send() {
    const v = text.trim();
    if (!canSend) return;
    const sent = onsend(v, ready);
    for (const a of ready) if (a.url) URL.revokeObjectURL(a.url);
    attachments = attachments.filter((a) => a.status === "failed");
    text = "";
    drafts.delete(thread.id);
    // A send the server refused gives the words back (unless the person has already typed something else).
    if (sent) void sent.catch(() => { if (!text.trim()) text = v; });
  }
</script>

<Panel class="composer" data-narrow={sheet ? "" : undefined} radius="pane" data-ring-host bind:el={panelEl} data-collapsed={open ? undefined : ""} onfocusin={(e) => { const t = e.target as HTMLElement; if (t.matches(":focus-visible") || t.closest("[contenteditable], textarea, input:not([type=file])")) focused = true; }} onclick={tapBox} onfocusout={(e) => { if (e.relatedTarget instanceof Node && panelEl?.contains(e.relatedTarget)) return; queueMicrotask(() => (focused = !!panelEl?.contains(document.activeElement))); }} onpointerdown={press} {onpaste} {ondrop} {ondragover} ondragleave={() => (dropping = false)} data-dropping={dropping ? "" : undefined}>
  <div class="ctx" data-fold aria-label="Context">
    {#if thread.workspace}<span class="cx"><Icon inline name="folder" /><span>{thread.workspace}</span></span>{/if}
    <ActiveChildren id={thread.id} />
    {#each mentions as m}<span class="cx on"><Icon inline name={m.label === provider?.name || pool.find((p) => p.label === m.label)?.kind === "agent" ? "spark" : pool.find((p) => p.label === m.label)?.kind === "thread" ? "threads" : pool.find((p) => p.label === m.label)?.kind === "item" ? "notes" : "user"} /><span>{m.label}</span><button type="button" class="x" aria-label="Remove {m.label}" onclick={() => dropMention(m)}><Icon inline name="close" /></button></span>{/each}
  </div>
  {#if attachments.length}
    <div class="atts" role="list" aria-label="Attachments">
      {#each attachments as a (a.id)}
        <div class="att" role="listitem" class:file={a.kind === "file"} class:failed={a.status === "failed"} title={a.status === "failed" ? `${a.name}: ${a.error ?? "not uploaded"}` : `${a.name}, ${size(a.size)}`}>
          {#if a.url}<img src={a.url} alt="" />{:else}<span class="ico"><Icon name="file" /></span><span class="nm">{a.name}</span><span class="sz">{size(a.size)}</span>{/if}
          {#if a.status === "uploading"}<span class="prog" role="progressbar" aria-label="Uploading {a.name}" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(a.progress * 100)} style:--p={a.progress}></span>{/if}
          {#if a.status === "failed"}<span class="bad" role="status"><Icon name="warning" inline />{a.error ?? "Not uploaded"}</span>{/if}
          <button type="button" class="rm" aria-label="Remove {a.name}" onclick={() => removeAttachment(a.id)}><Icon name="close" inline /></button>
        </div>
      {/each}
    </div>
  {/if}
  <input bind:this={picker} type="file" multiple hidden onchange={(e) => { addFiles([...(e.currentTarget.files ?? [])]); e.currentTarget.value = ""; }} />
  <div class="field" data-fold-row bind:this={fieldBox}>
    <InlineEditor id="composer-field" class="field-input" multiline label="Message" bind:el={field} bind:editor bind:value={text} {marks} triggers={{ kinds: ["mention", "skill"], on: (t) => (trig = t) }} onkeydown={onkey} onsubmit={() => send()} placeholder={running ? "Steer the running turn…" : `Message ${provider?.name ?? "the agent"}…`} />
    {#if (running || parked) && !text.trim()}
      <SendControl state="stop" label="Stop the turn" onclick={onstop} />
    {:else}
      <SendControl state={uploading ? "busy" : !canSend ? "idle" : running ? "steer" : "ready"} label={uploading ? "Uploading files" : running ? "Steer the turn" : "Send message"} onclick={send} />
    {/if}
  </div>
  <!-- The four chips (docs/composer.md, "Chips and adaptive rows"): the row measures its own room and gives up words in the stated order; every chip is icon and text or icon only, never two facts joined by a dot. -->
  <div class="chips" data-fold>
    <PillRow ids={CHIPS} drop={["where", "perm", "model"]} keep={keepWords} floor={400} label="Message options">
      {#snippet pill(id, compact, probe)}
        {#if probe}
          <!-- Only measured, never seen or used: no bindings, menus or handlers (the real pill below owns those). -->
          {#if id === "attach"}<span class="chip"><Pill label="Add photos and files" icon="plus" glass={false} /></span>
          {:else if id === "model"}<span class="chip"><Pill label={modelFull} icon={providerIcon} showLabel={!compact} glass={false}><span class="lbl">{modelName}</span>{#if thread.reasoning && levelsOf.length}<LevelMark level={thread.reasoning} levels={levelsOf} />{/if}</Pill></span>
          {:else if id === "perm"}<span class="chip"><Pill label={permissionLabel(thread.permission)} icon={PERM_ICON} showLabel={!compact} glass={false}><span class="lbl">{permissionLabel(thread.permission)}</span></Pill></span>
          {:else}<span class="chip"><Pill label={runsOnLabel(thread.runsOn)} icon="terminal" showLabel={!compact} glass={false}><span class="lbl">{thread.runsOn.machine}</span>{#if thread.runsOn.mode === "sandboxed"}<Icon inline name="shield" />{/if}</Pill></span>{/if}
        {:else}
        {#if id === "attach"}
          <span class="chip" data-chip="attach"><Pill label="Add photos and files" icon="plus" glass={false} aria-haspopup="menu" aria-expanded={attachOpen} bind:el={attachTrigger} onclick={() => { attachOpen = !attachOpen; }} />
            <Menu open={attachOpen} items={attachItems} morph label="Add to the message" trigger={attachTrigger} onclose={() => (attachOpen = false)} onpick={pickAttach} /></span>
        {:else if id === "model"}
          <span class="chip" data-chip="model"><Pill label={modelFull} icon={providerIcon} showLabel={!compact} glass={false} aria-haspopup="menu" aria-expanded={menuOpen} bind:el={trigger} onclick={() => { menuOpen = !menuOpen; }}>
            <span class="lbl">{modelName}</span>{#if thread.reasoning && levelsOf.length}<LevelMark level={thread.reasoning} levels={levelsOf} />{/if}
          </Pill>
            <Menu open={menuOpen} {items} {empty} search compact morph label="Provider, model and reasoning" trigger={trigger} onclose={() => (menuOpen = false)} onpick={pickModel} /></span>
        {:else if id === "perm"}
          <span class="chip" data-chip="perm" class:warn={thread.permission === "full"}><Pill label={running ? `${permissionLabel(thread.permission)}, applies to the next turn` : permissionLabel(thread.permission)} icon={PERM_ICON} showLabel={!compact} glass={false} aria-haspopup="menu" aria-expanded={permOpen} bind:el={permTrigger} onclick={() => { permOpen = !permOpen; }}>
            <span class="lbl">{permissionLabel(thread.permission)}</span>
          </Pill>
            <Menu open={permOpen} items={permItems} morph label={running ? "Permission mode for the next turn" : "Permission mode"} trigger={permTrigger} onclose={() => (permOpen = false)} onpick={(pid) => onpermission(pid as Permission)} /></span>
        {:else}
          <span class="chip" data-chip="where"><Pill label={runsOnLabel(thread.runsOn)} icon="terminal" showLabel={!compact} glass={false} aria-haspopup="dialog" aria-expanded={whereOpen} bind:el={whereTrigger} onclick={() => { whereOpen = !whereOpen; }}>
            <span class="lbl">{thread.runsOn.machine}</span>{#if thread.runsOn.mode === "sandboxed"}<Icon inline name="shield" />{/if}
          </Pill>
            {#if !sheet}<Popover open={whereOpen} anchor={whereTrigger} morph label="Where it runs" trigger={whereTrigger} onclose={() => (whereOpen = false)}><RunsOnPicker {machines} value={thread.runsOn} onchange={onrunson} /></Popover>{/if}</span>
        {/if}
        {/if}
      {/snippet}
    </PillRow>
  </div>
</Panel>

<MentionPicker {groups} bind:sel anchor={fieldBox} label={trig?.kind === "skill" ? "Skills" : "Mention"} onchoose={choose} onclose={() => (dismissed = trig?.from ?? null)} />

{#if sheet}
  <Sheet open={whereOpen} label="Where it runs" onclose={() => (whereOpen = false)}><RunsOnPicker {machines} value={thread.runsOn} onchange={onrunson} /><div class="done"><Button variant="primary" text="Done" onclick={() => (whereOpen = false)} /></div></Sheet>
{/if}

<style>
    :global(.composer) { --ring-offset: 2px; container: cmp / inline-size; padding: var(--s-3) var(--s-3) var(--s-3) var(--s-6); display: grid; grid-template-columns: minmax(0, 1fr); }
  /* Expand on focus, collapse on blur: the state flips at once (the two outer rows leave the layout when collapsed, and on a phone the box takes 26 px more inset a side, the
     measured ChatGPT/Claude recordings: 38 pt from the edges unfocused, 12 pt focused), and fold.ts plays the motion from the old look over --dur-morph on --ease-morph (the iPhone keyboard's own track,
     350 ms): the glass's edges, the field and the button by transform, the rows by opacity, so no layout runs per frame. Interruptible; instant in keyboard use and under reduced motion. */
  :global(.composer[data-collapsed]) [data-fold] { display: none; }
  
  :global(.composer[data-narrow][data-collapsed]) { margin-inline: calc(var(--s-8) + var(--s-2)); }
  .ctx { display: flex; flex-wrap: wrap; gap: var(--s-2); padding: 0 var(--s-3) var(--s-1) 0; }
  .cx { display: inline-flex; align-items: center; gap: var(--s-2); max-width: 100%; padding: var(--s-1) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-2); font-size: var(--text-label); }
  .cx > span:not(:first-child) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cx.on { background: color-mix(in oklab, var(--accent) 16%, transparent); color: var(--ink); }
  .x { display: inline-grid; place-items: center; color: var(--ink-3); border-radius: 50%; }
  .field { display: flex; align-items: flex-end; gap: var(--s-3); }
  .field > :global(.go) { margin-bottom: 0; }
  /* The caret, the placeholder and the typed text are one box: same font, same line height, same inset, so the caret sits exactly on the text's baseline.
     The height is a whole number of lines plus the vertical padding, never a fixed pixel height that leaves the text off centre. */
  :global(.field-input::placeholder) { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  :global(.field-input) { flex: 1; /* one line sits on the round control's centre: the padding is what is left of the control's height round the field's own line box (`1lh`), at every text size */ padding: max(var(--s-3), calc((var(--pill-h) - 1lh) / 2)) 0; overflow-y: auto; scrollbar-width: none; font-size: var(--text-ui); min-height: var(--pill-h); max-height: 9.5rem; }
    /* The chips row takes the composer's width; `PillRow` measures it. A chip is a quiet pill; Full access wears a warning tint so it is never hidden or mistaken. */
  .chips { min-width: 0; padding-right: var(--s-3); padding-top: var(--s-1); }
  .chip { position: relative; flex: none; display: inline-flex; }
  .chip :global(.pill) { max-width: 100%; }
  .lbl { white-space: nowrap; }
  .chip.warn :global(.pill) { color: var(--danger); background: color-mix(in oklab, var(--danger) 14%, transparent); }
  /* The attachments row: above the text, horizontally scrollable, each item a tile (a picture, or a file's icon and name) with a round remove button at its corner and a progress ring while it uploads. It opens by growing and its tiles fade in after it (the box first, the content a beat later, as in the apps this follows). */
  .atts { display: flex; gap: var(--s-4); overflow-x: auto; scrollbar-width: none; padding: var(--s-1) var(--s-4) var(--s-3) 0; animation: atts-in var(--dur-base) var(--ease-out); }
  @keyframes atts-in { from { opacity: 0; translate: 0 var(--s-3); } }
  .att { position: relative; flex: none; display: grid; grid-auto-flow: column; align-items: center; gap: var(--s-3); height: 64px; min-width: 64px; max-width: 220px; border-radius: var(--r-card); overflow: hidden; background: rgb(var(--ink-rgb) / 0.07); animation: att-in var(--dur-fast) var(--ease-out) var(--dur-base) both; }
  @keyframes att-in { from { opacity: 0; scale: 0.92; } }
  .att img { width: 64px; height: 64px; object-fit: cover; display: block; }
  .att.file { padding: 0 var(--s-9) 0 var(--s-5); color: var(--ink-2); font-size: var(--text-label); }
  .att .ico { display: grid; color: var(--ink-3); }
  .att .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 110px; }
  .att .sz { color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .att.failed { outline: 1px solid color-mix(in oklab, var(--danger) 60%, transparent); outline-offset: -1px; }
  .bad { position: absolute; inset: auto 0 0 0; padding: var(--s-1) var(--s-3); font-size: var(--text-section); color: var(--danger); background: color-mix(in oklab, var(--paper) 80%, transparent); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .prog { position: absolute; inset: 0; display: grid; place-items: center; background: rgb(0 0 0 / 0.35); }
  .prog::before { content: ""; width: 28px; height: 28px; border-radius: 50%; background: conic-gradient(#fff calc(var(--p) * 360deg), rgb(255 255 255 / 0.3) 0); -webkit-mask: radial-gradient(circle, transparent 9px, #000 10px); mask: radial-gradient(circle, transparent 9px, #000 10px); }
  .rm { position: absolute; top: var(--s-2); right: var(--s-2); display: grid; place-items: center; width: var(--s-8); height: var(--s-8); border-radius: 50%; color: #fff; background: rgb(0 0 0 / 0.55); }
  :global(.composer[data-dropping]) { outline: 2px dashed var(--accent); outline-offset: -2px; }
  .done { display: flex; justify-content: flex-end; padding: 0 var(--s-5) var(--s-5); }
</style>
