<script lang="ts">
  // A form node: named fields held on the device, checked here (required, length, url/email/number/date shape, choice among its options) before the submit enables; the values leave the device only in the submit's
  // interaction, keyed by field id. Typing is not consent: the host still previews every material field. After `ok` the fields clear (`clear: onOk`, the default); after error or an unknown outcome they keep what
  // was typed, and the error shows in a notice above the submit. The submit is aria-disabled while it runs.
  // 14.43: a field with `showWhen` is drawn and sent only while its condition on an EARLIER field of this form holds (a hidden field supplies no value and no required error); a form bound to an item view with
  // `entry_context` is disabled while the view is read-only, shows the reply or edit mode a message action opened (the edit text prefills a field whose `initial` is `{entry: "text"}`) and sends the host's composer ref;
  // a multiline field with `draft` keeps its text for the person through the host (read at first draw, saved shortly after typing, never in the shared thread); with `mentions` typing `@` offers only the item's
  // participants, and the picked choices travel as host-issued refs in `mention_choices`.
  import type { ActionOutcome, FormNode } from "../model";
  import { useApp, useActions } from "../invoke/context";
  import ActionButton from "./ActionButton.svelte";

  type Cond = { form: string; field: string; op: "exists" | "eq"; value?: unknown };
  type Field = FormNode["fields"][number] & { showWhen?: Cond; draft?: { slot: string }; mentions?: "item_participants"; initial?: unknown };
  type Node14 = Omit<FormNode, "fields"> & { fields: Field[]; entry_context?: { component: string } };
  let { node: raw }: { node: FormNode } = $props();
  const node = $derived(raw as unknown as Node14);
  const app = useApp();
  const { viewContext, session } = useActions();
  const literal = (f: Field): unknown => (typeof f.initial === "object" && f.initial !== null ? "" : f.initial);
  const initial = () => Object.fromEntries(node.fields.map((f) => [f.id, literal(f) ?? (f.input === "toggle" ? false : "")]));
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
  const visible = $derived(node.fields.filter((f) => shown(f) && f.input !== ("file" as string)));
  const iv = $derived(node.entry_context ? app.itemViews.map.get(node.entry_context.component) : undefined);
  const readOnly = $derived(!!iv?.readOnly);
  const mode = $derived(iv?.composer ?? null);
  const replyTo = $derived(mode ? iv?.entryOf(mode.anchor) : undefined);
  const valid = $derived(!readOnly && node.fields.every((f) => problem(f, values[f.id]) === null));
  const submit = $derived(node.submit);
  const context = $derived(viewContext(submit.id));
  /** A form of one multiline field is a message composer: the field and its submit sit on one line, Enter sends and Shift+Enter starts a new line (never while an input method is composing). */
  const isCompose = $derived(node.fields.filter((f) => f.input === "multiline").length === 1 && node.fields.every((f) => f.input === "multiline" || f.input === "toggle"));
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

  const interaction = () => {
    const sent = Object.fromEntries(Object.entries(values).filter(([k]) => { const f = node.fields.find((x) => x.id === k); return !f || shown(f); }));
    const text = mentionField ? String(sent[mentionField.id] ?? "") : "";
    const refs = chosen.filter((c) => text.includes(`@${c.label}`)).map((c) => c.ref).slice(0, 16);
    return {
      form: (session.formKeyed ? { [node.id]: sent } : sent) as Record<string, unknown>,
      ...(mode ? { entry_composer: mode.ref } : {}),
      ...(mentionField && refs.length ? { mention_choices: { [mentionField.id]: refs } } : {}),
    } as never;
  };
  function done(o: unknown) {
    const out = o as ActionOutcome | { state: string };
    if (out.state === "ok") { error = null; if ((node.clear ?? "onOk") === "onOk") { values = initial(); chosen = []; } if (iv) iv.composer = null; }
    else if (out.state === "error" || out.state === "refused" || out.state === "stale" || out.state === "unknown") error = (out as ActionOutcome).message ?? "That did not work.";
  }
</script>

<form class="form" class:compose={isCompose} data-node="form" data-form={node.id} bind:this={formEl} onsubmit={(e) => e.preventDefault()}>
  {#if node.title}<h3 class="ft">{node.title}</h3>{/if}
  {#if mode}<div class="mode" data-composer-mode={mode.mode}><span>{mode.mode === "reply" ? `Replying to ${replyTo?.author.name ?? "a message"}` : "Editing your message"}</span><button type="button" aria-label="Cancel" data-composer-cancel onclick={() => { const was = mode.mode; if (iv) iv.composer = null; if (was === "edit" && editField) values[editField.id] = ""; }}>✕</button></div>{/if}
  {#each visible as f (f.id)}
    <label class="f"><span class="l" class:sr={isCompose && f.input === "multiline"}>{f.label}{#if f.required}<span aria-hidden="true"> *</span>{/if}</span>
      {#if f.input === "multiline"}<textarea maxlength={f.maxLength ?? 200} placeholder={f.placeholder} rows={isCompose ? 1 : undefined} aria-label={f.label} disabled={readOnly} bind:value={values[f.id] as string}
        onkeydown={isCompose || f.mentions ? enterSends : undefined} oninput={(e) => typed(f, e.currentTarget)}></textarea>
      {:else if f.input === "choice"}<select bind:value={values[f.id] as string}><option value=""></option>{#each f.options ?? [] as o (o.value)}<option value={o.value}>{o.label}</option>{/each}</select>
      {:else if f.input === "toggle"}<input type="checkbox" bind:checked={values[f.id] as boolean} />
      {:else}<input type={f.input === "url" ? "url" : f.input === "email" ? "email" : f.input === "number" ? "text" : f.input === "date" ? "date" : "text"} inputmode={f.input === "number" ? "decimal" : undefined} maxlength={f.maxLength ?? 200} placeholder={f.placeholder} bind:value={values[f.id] as string} />{/if}
    </label>
  {/each}
  {#if picker}<ul class="pick" role="listbox" aria-label="Mention a participant" data-mention-picker>{#each picker.options as o, k (o.ref)}<li><button type="button" role="option" aria-selected={k === picker.sel} data-kind={o.kind} onclick={() => choose(o)}>{o.label}<span class="k">{o.kind === "contribution" ? "agent" : ""}</span></button></li>{/each}</ul>{/if}
  {#if error}<div class="err" role="alert" data-form-error>{error}</div>{/if}
  <ActionButton action={submit} {context} {interaction} style="primary" disabled={!valid} onrun={done} />
</form>

<style>
  .form { display: grid; gap: var(--s-3); position: relative; }
  .ft { margin: 0; font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .f { display: grid; gap: var(--s-1); } .l { font-size: var(--text-label); color: var(--ink-3); }
  input:not([type=checkbox]), textarea, select { min-height: 44px; padding: 0 var(--s-4); border-radius: var(--r-select); border: 0; background: color-mix(in oklab, var(--ink) 8%, transparent); color: var(--ink); font-size: 16px; }
  textarea { padding: var(--s-3) var(--s-4); } textarea:disabled { opacity: 0.5; }
  .compose { grid-template-columns: minmax(0, 1fr) auto; align-items: end; }
  .compose > .err, .compose > .mode, .compose > .pick { grid-column: 1 / -1; } .compose textarea { min-height: 44px; max-height: 40vh; field-sizing: content; resize: none; }
  .compose .f:has(input[type=checkbox]) { grid-column: 1 / -1; grid-auto-flow: column; justify-content: start; align-items: center; gap: var(--s-3); }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  .mode { display: flex; align-items: center; justify-content: space-between; gap: var(--s-3); padding: var(--s-2) var(--s-4); border-inline-start: 3px solid var(--accent); border-radius: 4px; background: color-mix(in oklab, var(--ink) 6%, transparent); color: var(--ink-2); }
  .mode button { min-width: 44px; min-height: 44px; color: var(--ink-2); }
  .pick { list-style: none; margin: 0; padding: var(--s-2); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 10%, var(--paper, #fff)); max-height: 40vh; overflow: auto; }
  .pick button { width: 100%; min-height: 44px; text-align: start; padding: 0 var(--s-4); border-radius: var(--r-select); color: var(--ink); display: flex; justify-content: space-between; } .pick button[aria-selected="true"], .pick button:hover { background: var(--hover); } .k { color: var(--ink-3); font-size: var(--text-label); }
  .err { padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--danger) 12%, transparent); color: var(--danger); }
</style>
