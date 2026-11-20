<script lang="ts">
  // A form derived from an operation: one field per input the action did not supply, labelled and typed from the operation
  // (or, for a declarative create, from the kind's fields), prefilled from the action's `defaults`, plus a picker for each
  // input the action fills from a slot's parts. A `confirm` sentence makes it a confirmation instead. Submit is one call.
  import { Button, Sheet } from "@calternal/ui";
  import { opOf, slots, type Json } from "../lib/model";
  import { formFields, type Action } from "../lib/look";
  import { call } from "../lib/calls.svelte";
  let { action, kind, prefill = {}, onclose }: { action: Action; kind?: string; prefill?: Record<string, string>; onclose: () => void } = $props();
  const op: Json = $derived(opOf(action.op) ?? {});
  const fields = $derived(formFields(op, { ...action.inputs, ...(action.pick ?? {}) }, kind));
  const picks = $derived(Object.entries(action.pick ?? {}).map(([name, slot]) => ({ name, parts: (slots[slot] ?? []) as Json[] })));
  const values = $state<Record<string, string>>({});
  $effect.pre(() => { for (const [k, v] of Object.entries(prefill)) values[k] ??= v; for (const p of picks) values[p.name] ??= p.parts[0]?.plugin ?? ""; });
  const submit = (e: Event) => { e.preventDefault(); call(action.op, { ...action.inputs, ...values }); onclose(); };
  const narrow = matchMedia("(max-width: 719px)").matches;
</script>

<Sheet open label={op.title ?? action.label} {onclose} side={narrow ? "bottom" : "center"}>
  <form class="f" onsubmit={submit}>
    <h2>{action.confirm ?? op.title ?? action.label}</h2>
    {#each fields as f (f.name)}
      <label><span>{f.label}{#if f.optional}<em> (optional)</em>{/if}</span>
        {#if f.name === "text" || f.name === "prompt"}<textarea rows="3" bind:value={values[f.name]} required={!f.optional}></textarea>
        {:else}<input type={f.type === "url" ? "url" : "text"} bind:value={values[f.name]} required={!f.optional} />{/if}
      </label>
    {/each}
    {#each picks as p (p.name)}
      <label><span>{p.name[0]!.toUpperCase() + p.name.slice(1)}</span>
        <select bind:value={values[p.name]}>{#each p.parts as part (part.plugin)}<option value={part.plugin}>{part.name}</option>{/each}</select>
      </label>
    {/each}
    <div class="btns"><Button text="Cancel" variant="quiet" onclick={onclose} /><Button type="submit" text={action.confirm ? "Delete" : (op.title ?? action.label)} variant={action.confirm ? "destructive" : "primary"} /></div>
  </form>
</Sheet>

<style>
  .f { display: grid; gap: var(--s-5); padding: var(--s-6); min-width: min(26rem, 90vw); }
  h2 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); }
  label { display: grid; gap: var(--s-2); } label span { font-size: var(--text-label); color: var(--ink-2); } em { color: var(--ink-3); font-style: normal; }
  input, textarea, select { padding: var(--s-4); border: 0; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font: inherit; resize: vertical; }
  .btns { display: flex; justify-content: flex-end; gap: var(--s-3); }
</style>
