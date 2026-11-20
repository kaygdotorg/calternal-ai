<script lang="ts">
  // A form node: named fields held on the device, checked here (required, length, url/email/number/date shape, choice among its options) before the submit enables; the values leave the device only in the submit's
  // interaction, keyed by field id. Typing is not consent: the host still previews every material field. After `ok` the fields clear (`clear: onOk`, the default); after error or an unknown outcome they keep what
  // was typed, and the error shows in a notice above the submit. The submit is aria-disabled while it runs.
  // 14.43: a field with `showWhen` is drawn and sent only while its condition on an EARLIER field of this form holds (a hidden field supplies no value and no required error); a form bound to an item view with
  // `entry_context` is disabled while the view is read-only, shows the reply or edit mode a message action opened (the edit text prefills a field whose `initial` is `{entry: "text"}`) and sends the host's composer ref;
  // a multiline field with `draft` keeps its text for the person through the host (read at first draw, saved shortly after typing, never in the shared thread); with `mentions` typing `@` offers only the item's
  // participants, and the picked choices travel as host-issued refs in `mention_choices`.
  import type { ActionOutcome, FormNode } from "../model";
  import { Icon, Panel, Pill } from "@calternal/ui";
  import { useApp, useActions } from "../invoke/context";
  import ActionButton from "./ActionButton.svelte";

  type Cond = { form: string; field: string; op: "exists" | "eq"; value?: unknown };
  type Field = FormNode["fields"][number] & { showWhen?: Cond; draft?: { slot: string }; mentions?: "item_participants"; initial?: unknown };
  type Node14 = Omit<FormNode, "fields"> & { fields: Field[]; entry_context?: { component: string } };
  let { node: raw }: { node: FormNode } = $props();
  const node = $derived(raw as unknown as Node14);
  const app = useApp();
  const { viewContext, session, ui } = useActions();
  const literal = (f: Field): unknown => (typeof f.initial === "object" && f.initial !== null ? "" : f.initial);
  const initial = () => Object.fromEntries(node.fields.map((f) => [f.id, literal(f) ?? (f.input === "toggle" ? false : (f.input as string) === "file" ? [] : "")]));
  let values = $state<Record<string, unknown>>(initial());
  let error = $state<string | null>(null);
  const shapes: Record<string, RegExp> = { url: /^https?:\/\/\S+$/i, email: /^[^\s@]+@[^\s@]+\.[^\s@]+$/, number: /^-?\d+(\.\d+)?$/, date: /^\d{4}-\d{2}-\d{2}$/ };
  /** Whether a field's condition holds on the values typed so far (always, without one). */
  function shown(f: Field): boolean {
    const c = f.showWhen; if (!c) return true;
    const v = values[c.field];
    return c.op === "exists" ? v !== undefined && v !== null && v !== "" && v !== false : v === c.value || String(v) === String(c.value);
  }
  /** The first problem with a field's value, or null. */
  function problem(f: Field, v: unknown): string | null {
    if (!shown(f)) return null;
    const s = typeof v === "string" ? v : "";
    if (f.input === "toggle") return null;
    if (f.required && s.trim() === "") return `${f.label} is required.`;
    if (s === "") return null;
    if (s.length > (f.maxLength ?? 200)) return `${f.label} is too long.`;
    if (shapes[f.input] && !shapes[f.input]!.test(s)) return `${f.label} is not a valid ${f.input}.`;
    if (f.input === "choice" && !(f.options ?? []).some((o) => o.value === s)) return `Choose one of the options for ${f.label}.`;
    return null;
  }
  const visible = $derived(node.fields.filter((f) => shown(f) && (f.input as string) !== "file"));
  const iv = $derived(node.entry_context ? app.itemViews.map.get(node.entry_context.component) : undefined);
  const readOnly = $derived(!!iv?.readOnly);
  const mode = $derived(iv?.composer ?? null);
  const replyTo = $derived(mode ? iv?.entryOf(mode.anchor) : undefined);
  // (attachment uploads, below: their state is needed here)
  type Up = { id: string; name: string; bytes: number; ref?: string; sent: number; error?: string };
  let ups = $state<Record<string, Up[]>>({}), fileEl = $state<HTMLInputElement>();
  const fileFields = $derived(node.fields.filter((f) => (f.input as string) === "file"));
  const uploading = $derived(Object.values(ups).some((l) => l.some((u) => !u.ref && !u.error)));
  const valid = $derived(!readOnly && !uploading && node.fields.every((f) => problem(f, values[f.id]) === null));
  const submit = $derived(node.submit);
  const context = $derived(viewContext(submit.id));
  /** A form of one multiline field is a message composer: the field and its submit sit on one line, Enter sends and Shift+Enter starts a new line (never while an input method is composing). */
  const isCompose = $derived(node.fields.filter((f) => f.input === "multiline").length === 1 && node.fields.every((f) => f.input === "multiline" || f.input === "toggle" || (f.input as string) === "file"));
  let formEl = $state<HTMLFormElement>();
  function enterSends(e: KeyboardEvent) {
    if (picker && ["ArrowDown", "ArrowUp", "Enter", "Escape", "Tab"].includes(e.key)) { e.preventDefault(); keyPicker(e.key); return; }
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing && valid) { e.preventDefault(); formEl?.querySelector<HTMLElement>("[data-action]")?.click(); }
  }

  // ---- reply and edit mode
  const editField = $derived(node.fields.find((f) => typeof f.initial === "object" && f.initial !== null && (f.initial as { entry?: string }).entry === "text"));
  $effect(() => { const m = mode; if (m?.mode === "edit" && editField && m.initial !== undefined) values[editField.id] = m.initial; });

  // ---- drafts kept for the person by the host (one slot per field)
  const draftField = $derived(node.fields.find((f) => f.draft));
  const windowRef = $derived(iv?.reply?.window_ref);
  let draftVersion: string | undefined, draftLoadedFor = "", draftTimer = 0, applying = false;
  $effect(() => {
    const f = draftField, w = windowRef; if (!f || !w || draftLoadedFor === w) return;
    draftLoadedFor = w;
    void session.exchange<{ version: string; text: string }>("entry_composer.draft.read", { type: "draft_read", window_ref: w, form: node.id, field: f.id }).then((d) => { draftVersion = d.version; if (d.text && !values[f.id]) { applying = true; values[f.id] = d.text; applying = false; } }, () => {});
  });
  $effect(() => {
    const f = draftField; if (!f) return;
    const text = String(values[f.id] ?? ""); void text;
    if (applying || !windowRef) return;
    clearTimeout(draftTimer);
    draftTimer = window.setTimeout(() => {
      void session.exchange<{ version: string }>("entry_composer.draft", { type: "draft_update", window_ref: windowRef!, form: node.id, field: f.id, expected: draftVersion ?? "", request_id: crypto.randomUUID(), text: text.slice(0, 4000) }).then((d) => { draftVersion = d.version; }, () => {});
    }, 800);
    return () => clearTimeout(draftTimer);
  });

  // ---- mentions: only the item's participants, as host-issued choice refs
  const mentionField = $derived(node.fields.find((f) => f.mentions));
  let picker = $state<{ options: { ref: string; label: string; kind: string }[]; at: number; sel: number } | null>(null), chosen = $state<{ ref: string; label: string }[]>([]), mseq = 0;
  async function typed(f: Field, el: HTMLTextAreaElement) {
    if (f !== mentionField || !windowRef) return;
    const upto = el.value.slice(0, el.selectionStart ?? el.value.length), m = /(?:^|\s)@([^\s@]{0,40})$/.exec(upto);
    if (!m) { picker = null; return; }
    const my = ++mseq;
    try { const r = await session.exchange<{ options: { ref: string; label: string; kind: string }[] }>("entry_composer.mentions", { type: "entry_mentions", window_ref: windowRef, form: node.id, field: f.id, text: m[1]! }); if (my === mseq) picker = r.options.length ? { options: r.options, at: upto.length - m[1]!.length - 1, sel: 0 } : null; } catch { picker = null; }
  }
  function choose(o: { ref: string; label: string }) {
    const f = mentionField!, v = String(values[f.id] ?? ""), p = picker!;
    const end = p.at + 1 + (/^[^\s@]*/.exec(v.slice(p.at + 1))?.[0].length ?? 0);
    values[f.id] = `${v.slice(0, p.at)}@${o.label} ${v.slice(end)}`;
    if (!chosen.some((c) => c.ref === o.ref)) chosen = [...chosen, o];
    picker = null;
  }
  function keyPicker(k: string) {
    if (!picker) return;
    if (k === "ArrowDown") picker.sel = (picker.sel + 1) % picker.options.length;
    else if (k === "ArrowUp") picker.sel = (picker.sel - 1 + picker.options.length) % picker.options.length;
    else if (k === "Escape") picker = null;
    else choose(picker.options[picker.sel]!);
  }

  // ---- composer chips: the agent and where it runs, picked ahead of Send (the web app's chips) instead of asked at Send. A chip is the choice prompt of the submit action; it asks the host for its options on the submit's own
  // context, and what was picked travels in the send with the activation id a placement option was fetched for. A prompt not picked on a chip is still asked at Send.
  type Pick = { ref: string; label: string; harness?: string };
  type PromptSpec = { title: string; kind?: string; options?: { extension?: string; host?: string } | unknown; showWhen?: Cond };
  const prompts = $derived(Object.entries(node.submit.inputs ?? {}).flatMap(([name, src]) => { const p = (src as { prompt?: PromptSpec }).prompt; return p && p.kind === "choice" ? [{ name, p }] : []; }).filter(({ p }) => !p.showWhen || shown({ showWhen: p.showWhen } as Field)));
  let picks = $state<Record<string, Pick>>({}), actId = $state(crypto.randomUUID());
  const harnessOf = $derived(Object.values(picks).find((c) => c.harness)?.harness);
  const optsOf = (p: PromptSpec) => (p.options && !Array.isArray(p.options) ? (p.options as { extension?: string; host?: string }) : {});
  async function page(name: string, p: PromptSpec) {
    const from = optsOf(p), ctx = viewContext(submit.id);
    if (!ctx) return null;
    type P = { options: { ref: string; label: string; harness_ref?: string }[]; default_ref?: string };
    if (from.extension) return session.exchange<P>("extension.choices", { type: "extension_choices", app: session.app ?? "", point: from.extension, activation_context: ctx, slot: name });
    return session.exchange<P>("choice.options", { type: "choice_options", activation_context: ctx, slot: name, ...(from.host ? { activation_id: actId, ...(harnessOf ? { harness: harnessOf } : {}) } : {}) });
  }
  async function pickFor(name: string, p: PromptSpec) {
    const pg = await page(name, p).catch(() => null); if (!pg) return;
    const ref = await ui.choose({ name, title: p.title, options: pg.options, default_ref: pg.default_ref }); if (ref === null) return;
    const o = pg.options.find((x) => x.ref === ref)!;
    picks[name] = { ref, label: o.label, harness: o.harness_ref };
    if (o.harness_ref) for (const q of prompts) if (optsOf(q.p).host) { delete picks[q.name]; void preselect(q.name, q.p); }
  }
  /** The host's eligible default for a placement is shown on its chip at once, so Send needs no question. */
  async function preselect(name: string, p: PromptSpec) {
    const pg = await page(name, p).catch(() => null); const d = pg?.options.find((x) => x.ref === pg.default_ref);
    if (d && !picks[name]) picks[name] = { ref: d.ref, label: d.label };
  }
  $effect(() => { const h = harnessOf; if (!isCompose || !h) return; for (const q of prompts) if (optsOf(q.p).host && !picks[q.name]) void preselect(q.name, q.p); });
  $effect(() => { for (const n of Object.keys(picks)) if (!prompts.some((q) => q.name === n)) delete picks[n]; }); // a chip whose prompt is hidden again forgets its pick
  const preset = () => (isCompose && Object.keys(picks).length ? { activation_id: actId, harness: harnessOf } : undefined);
  const chipIcon = (p: PromptSpec) => (optsOf(p).host ? "terminal" : "spark");
  // ---- attachments: a `file` field uploads each chosen file through the host (begin, 64 KiB chunks, finish) and holds the checked attachment refs; Send waits while one is still going.
  const b64 = (u: Uint8Array) => { let s = ""; for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode(...u.subarray(i, i + 0x8000)); return btoa(s); };
  const hex = (b: ArrayBuffer) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, "0")).join("");
  async function upload(f: Field, files: FileList | null) {
    const w = windowRef; if (!files || !w) return;
    for (const file of [...files]) {
      ups[f.id] ??= []; const list = ups[f.id]!; list.push({ id: crypto.randomUUID(), name: file.name, bytes: file.size, sent: 0 });
      const u = list[list.length - 1]!; // (the reactive copy: its progress and result redraw the tray)
      try {
        const buf = await file.arrayBuffer();
        let r = await session.exchange<{ stage: string; next_offset: number; max_bytes: number }>("attachment.begin", { type: "attachment_begin", window_ref: w, form: node.id, field: f.id, request_id: u.id, name: file.name.slice(0, 120), bytes: file.size, sha256: hex(await crypto.subtle.digest("SHA-256", buf)) });
        if (file.size > r.max_bytes) throw new Error("too large");
        for (let off = r.next_offset; off < file.size; off = r.next_offset) { r = await session.exchange("attachment.chunk", { type: "attachment_chunk", stage: r.stage, offset: off, bytes: b64(new Uint8Array(buf, off, Math.min(65536, file.size - off))) }); u.sent = r.next_offset; }
        const done = await session.exchange<{ attachment: string }>("attachment.finish", { type: "attachment_finish", stage: r.stage });
        u.ref = done.attachment; u.sent = file.size;
        values[f.id] = (ups[f.id] ?? []).flatMap((x) => (x.ref ? [x.ref] : []));
      } catch (e) { u.error = e instanceof Error && e.message === "too large" ? "Too large" : "Not uploaded"; }
    }
  }
  function dropUp(f: Field, id: string) { ups[f.id] = (ups[f.id] ?? []).filter((x) => x.id !== id); values[f.id] = ups[f.id]!.flatMap((x) => (x.ref ? [x.ref] : [])); }
  const toggles = $derived(node.fields.filter((f) => f.input === "toggle" && shown(f)));
  const interaction = () => {
    const sent = Object.fromEntries(Object.entries(values).filter(([k]) => { const f = node.fields.find((x) => x.id === k); return !f || shown(f); }));
    const text = mentionField ? String(sent[mentionField.id] ?? "") : "";
    const refs = chosen.filter((c) => text.includes(`@${c.label}`)).map((c) => c.ref).slice(0, 16);
    return {
      form: (session.formKeyed ? { [node.id]: sent } : sent) as Record<string, unknown>,
      ...(mode ? { entry_composer: mode.ref } : {}),
      ...(isCompose && Object.keys(picks).length ? { choices: Object.fromEntries(Object.entries(picks).map(([k, v]) => [k, v.ref])) } : {}),
      ...(mentionField && refs.length ? { mention_choices: { [mentionField.id]: refs } } : {}),
    } as never;
  };
  function done(o: unknown) {
    const out = o as ActionOutcome | { state: string };
    if (out.state === "ok") { error = null; if ((node.clear ?? "onOk") === "onOk") { values = initial(); chosen = []; ups = {}; } if (iv) iv.composer = null; actId = crypto.randomUUID(); for (const k of Object.keys(picks)) if (optsOf(prompts.find((q) => q.name === k)?.p ?? { title: "" }).host) delete picks[k]; }
    else if (out.state === "error" || out.state === "refused" || out.state === "stale" || out.state === "unknown") error = (out as ActionOutcome).message ?? "That did not work.";
  }
</script>

{#snippet fieldsList()}
  {#each visible as f (f.id)}
    {#if !(isCompose && f.input === "toggle")}
    <label class="f"><span class="l" class:sr={isCompose && f.input === "multiline"}>{f.label}{#if f.required}<span aria-hidden="true"> *</span>{/if}</span>
      {#if f.input === "multiline"}<textarea maxlength={f.maxLength ?? 200} placeholder={f.placeholder} rows={isCompose ? 1 : undefined} aria-label={f.label} disabled={readOnly} bind:value={values[f.id] as string}
        onkeydown={isCompose || f.mentions ? enterSends : undefined} oninput={(e) => typed(f, e.currentTarget)}></textarea>
      {:else if f.input === "choice"}<select bind:value={values[f.id] as string}><option value=""></option>{#each f.options ?? [] as o (o.value)}<option value={o.value}>{o.label}</option>{/each}</select>
      {:else if f.input === "toggle"}<input type="checkbox" bind:checked={values[f.id] as boolean} />
      {:else}<input type={f.input === "url" ? "url" : f.input === "email" ? "email" : f.input === "number" ? "text" : f.input === "date" ? "date" : "text"} inputmode={f.input === "number" ? "decimal" : undefined} maxlength={f.maxLength ?? 200} placeholder={f.placeholder} bind:value={values[f.id] as string} />{/if}
    </label>
    {/if}
  {/each}
{/snippet}
{#snippet modeBar()}{#if mode}<div class="mode" data-composer-mode={mode.mode}><span>{mode.mode === "reply" ? `Replying to ${replyTo?.author.name ?? "a message"}` : "Editing your message"}</span><button type="button" aria-label="Cancel" data-composer-cancel onclick={() => { const was = mode.mode; if (iv) iv.composer = null; if (was === "edit" && editField) values[editField.id] = ""; }}><Icon name="close" /></button></div>{/if}{/snippet}
{#snippet pickList()}{#if picker}<ul class="pick" role="listbox" aria-label="Mention a participant" data-mention-picker>{#each picker.options as o, k (o.ref)}<li><button type="button" role="option" aria-selected={k === picker.sel} data-kind={o.kind} onclick={() => choose(o)}>{o.label}<span class="k">{o.kind === "contribution" ? "agent" : ""}</span></button></li>{/each}</ul>{/if}{/snippet}

{#if isCompose}
  <Panel radius="pane" class="composer" data-ring-host>
    <form class="form compose" data-node="form" data-form={node.id} bind:this={formEl} onsubmit={(e) => e.preventDefault()}>
      {@render modeBar()}
      {#each fileFields as f (f.id)}{#if ups[f.id]?.length}<ul class="trayu" aria-label="Attachments">{#each ups[f.id]! as u (u.id)}<li class="u" class:err={!!u.error} data-upload={u.ref ? "done" : u.error ? "error" : "going"}><Icon name="paperclip" inline /><span class="un">{u.name}</span>{#if u.error}<span class="ue" role="alert">{u.error}</span>{:else if !u.ref}<progress value={u.sent} max={u.bytes} aria-label={`Uploading ${u.name}`}></progress>{/if}<button type="button" aria-label={`Remove ${u.name}`} onclick={() => dropUp(f, u.id)}><Icon name="close" inline /></button></li>{/each}</ul>{/if}{/each}
      <div class="field">{@render fieldsList()}<ActionButton action={submit} {context} {interaction} {preset} style="primary" disabled={!valid} onrun={done} round /></div>
      {@render pickList()}
      {#if error}<div class="err" role="alert" data-form-error>{error}</div>{/if}
      {#if toggles.length || prompts.length || fileFields.length}
        <div class="chips" role="group" aria-label="Message options">
          {#each fileFields as f (f.id)}<Pill label={f.label} icon="paperclip" glass={false} data-chip={f.id} disabled={readOnly || !windowRef} onclick={() => fileEl?.click()} /><input bind:this={fileEl} type="file" multiple hidden data-file-input={f.id} onchange={(e) => { void upload(f, e.currentTarget.files); e.currentTarget.value = ""; }} />{/each}
          {#each toggles as f (f.id)}<Pill label={f.label} icon="spark" showLabel glass={false} aria-pressed={!!values[f.id]} data-chip={f.id} onclick={() => (values[f.id] = !values[f.id])}><span>{f.label}</span></Pill>{/each}
          {#each prompts as q (q.name)}<Pill label={picks[q.name] ? `${q.p.title}: ${picks[q.name]!.label}` : q.p.title} icon={chipIcon(q.p)} showLabel glass={false} data-chip={q.name} disabled={!!optsOf(q.p).host && !harnessOf} onclick={() => pickFor(q.name, q.p)}><span>{picks[q.name]?.label ?? q.p.title}</span></Pill>{/each}
        </div>
      {/if}
    </form>
  </Panel>
{:else}
<form class="form" data-node="form" data-form={node.id} bind:this={formEl} onsubmit={(e) => e.preventDefault()}>
  {#if node.title}<h3 class="ft">{node.title}</h3>{/if}
  {@render modeBar()}
  {@render fieldsList()}
  {@render pickList()}
  {#if error}<div class="err" role="alert" data-form-error>{error}</div>{/if}
  <ActionButton action={submit} {context} {interaction} style="primary" disabled={!valid} onrun={done} />
</form>
{/if}

<style>
  .form { display: grid; gap: var(--s-3); position: relative; }
  .ft { margin: 0; font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .f { display: grid; gap: var(--s-1); } .l { font-size: var(--text-label); color: var(--ink-3); }
  input:not([type=checkbox]), textarea, select { min-height: var(--pill-h); padding: 0 var(--s-4); border-radius: var(--r-select); border: 0; background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font: inherit; font-size: var(--text-body); }
  textarea { padding: var(--s-3) var(--s-4); } textarea:disabled { opacity: 0.5; }
  /* The composer (the web app's pattern): one glass pane holding the field and the round send control, and under them a row of flat chips. */
  :global(.composer) { flex: none; }
  .compose { gap: var(--s-1); padding: var(--s-2) var(--s-2) var(--s-1) var(--s-5); }
  .compose .field { display: flex; align-items: flex-end; gap: var(--s-3); } .compose .field > :global(label.f) { flex: 1; min-width: 0; }
  .compose textarea { width: 100%; box-sizing: border-box; background: transparent; border-radius: 0; padding: max(var(--s-3), calc((var(--pill-h) - 1lh) / 2)) 0; min-height: var(--pill-h); max-height: 40vh; field-sizing: content; resize: none; outline: none; }
  .compose textarea::placeholder { color: var(--ink-3); }
  .trayu { list-style: none; margin: 0; padding: var(--s-1) var(--s-1) 0 0; display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .u { display: inline-flex; align-items: center; gap: var(--s-2); padding: var(--s-1) var(--s-1) var(--s-1) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); font-size: var(--text-label); max-width: 100%; }
  .u .un { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 14rem; } .u.err { color: var(--danger); } .u progress { width: 3rem; height: 4px; accent-color: var(--accent-base); } .u button { display: inline-grid; place-items: center; min-width: 28px; height: 28px; border-radius: 50%; color: var(--ink-2); }
  .chips { display: flex; flex-wrap: wrap; gap: var(--s-1); padding: 0 var(--s-1) var(--s-1) 0; margin-inline-start: calc(var(--s-3) * -1); } .chips :global(.pill) { font-size: var(--text-label); }
  .compose > .err, .compose > .mode, .compose > .pick { margin-inline-end: var(--s-2); }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  .mode { display: flex; align-items: center; justify-content: space-between; gap: var(--s-3); padding: var(--s-1) var(--s-1) var(--s-1) calc(var(--s-4) + 3px); margin-top: var(--s-2); position: relative; border-radius: 0 var(--r-select) var(--r-select) 0; background: var(--accent-soft); color: var(--ink-2); }
  .mode::before { content: ""; position: absolute; inset-block: 0; inset-inline-start: 0; width: 3px; background: var(--accent); }
  .mode button { display: inline-grid; place-items: center; min-width: var(--pill-h); min-height: var(--pill-h); color: var(--ink-2); border-radius: 50%; }
  .pick { list-style: none; margin: 0; padding: var(--s-2); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.07); max-height: 40vh; overflow: auto; }
  .pick button { width: 100%; min-height: var(--row-h); text-align: start; padding: 0 var(--s-4); border-radius: var(--r-select); color: var(--ink); display: flex; justify-content: space-between; } .pick button[aria-selected="true"], .pick button:hover { background: var(--hover); } .k { color: var(--ink-3); font-size: var(--text-label); }
  .err { padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--danger) 12%, transparent); color: var(--danger); }
</style>
