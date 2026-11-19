<script lang="ts">
  // A form node: named fields held on the device, checked here (required, length, url/email/number/date shape, choice among its options) before the submit enables; the values leave the device only in the submit's
  // interaction, keyed by field id. Typing is not consent: the host still previews every material field. After `ok` the fields clear (`clear: onOk`, the default); after error or an unknown outcome they keep what
  // was typed, and the error shows in a notice above the submit. The submit is aria-disabled while it runs.
  import type { ActionOutcome, FormNode } from "../model";
  import { useActions } from "../invoke/context";
  import ActionButton from "./ActionButton.svelte";

  let { node }: { node: FormNode } = $props();
  const { session } = useActions();
  const initial = () => Object.fromEntries(node.fields.map((f) => [f.id, f.initial ?? (f.input === "toggle" ? false : "")]));
  let values = $state<Record<string, unknown>>(initial());
  let error = $state<string | null>(null);
  const shapes: Record<string, RegExp> = { url: /^https?:\/\/\S+$/i, email: /^[^\s@]+@[^\s@]+\.[^\s@]+$/, number: /^-?\d+(\.\d+)?$/, date: /^\d{4}-\d{2}-\d{2}$/ };
  /** The first problem with a field's value, or null. */
  function problem(f: FormNode["fields"][number], v: unknown): string | null {
    const s = typeof v === "string" ? v : "";
    if (f.input === "toggle") return null;
    if (f.required && s.trim() === "") return `${f.label} is required.`;
    if (s === "") return null;
    if (s.length > (f.maxLength ?? 200)) return `${f.label} is too long.`;
    if (shapes[f.input] && !shapes[f.input]!.test(s)) return `${f.label} is not a valid ${f.input}.`;
    if (f.input === "choice" && !(f.options ?? []).some((o) => o.value === s)) return `Choose one of the options for ${f.label}.`;
    return null;
  }
  const valid = $derived(node.fields.every((f) => problem(f, values[f.id]) === null));
  const submit = $derived(node.submit);
  const context = $derived(session.viewContexts[submit.id]);
  const interaction = () => ({ form: { ...values } as Record<string, unknown> });
  function done(o: unknown) {
    const out = o as ActionOutcome | { state: string };
    if (out.state === "ok") { error = null; if ((node.clear ?? "onOk") === "onOk") values = initial(); }
    else if (out.state === "error" || out.state === "refused" || out.state === "stale" || out.state === "unknown") error = (out as ActionOutcome).message ?? "That did not work.";
  }
</script>

<form class="form" data-node="form" data-form={node.id} onsubmit={(e) => e.preventDefault()}>
  {#if node.title}<h3 class="ft">{node.title}</h3>{/if}
  {#each node.fields as f (f.id)}
    <label class="f"><span class="l">{f.label}{#if f.required}<span aria-hidden="true"> *</span>{/if}</span>
      {#if f.input === "multiline"}<textarea maxlength={f.maxLength ?? 200} placeholder={f.placeholder} bind:value={values[f.id] as string}></textarea>
      {:else if f.input === "choice"}<select bind:value={values[f.id] as string}><option value=""></option>{#each f.options ?? [] as o (o.value)}<option value={o.value}>{o.label}</option>{/each}</select>
      {:else if f.input === "toggle"}<input type="checkbox" bind:checked={values[f.id] as boolean} />
      {:else}<input type={f.input === "url" ? "url" : f.input === "email" ? "email" : f.input === "number" ? "text" : f.input === "date" ? "date" : "text"} inputmode={f.input === "number" ? "decimal" : undefined} maxlength={f.maxLength ?? 200} placeholder={f.placeholder} bind:value={values[f.id] as string} />{/if}
    </label>
  {/each}
  {#if error}<div class="err" role="alert" data-form-error>{error}</div>{/if}
  <ActionButton action={submit} {context} {interaction} style="primary" disabled={!valid} onrun={done} />
</form>

<style>
  .form { display: grid; gap: var(--s-3); }
  .ft { margin: 0; font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .f { display: grid; gap: var(--s-1); } .l { font-size: var(--text-label); color: var(--ink-3); }
  input:not([type=checkbox]), textarea, select { min-height: 44px; padding: 0 var(--s-4); border-radius: var(--r-select); border: 0; background: color-mix(in oklab, var(--ink) 8%, transparent); color: var(--ink); font-size: 16px; }
  textarea { padding: var(--s-3) var(--s-4); }
  .err { padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--danger) 12%, transparent); color: var(--danger); }
</style>
