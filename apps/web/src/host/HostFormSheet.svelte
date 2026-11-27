<script lang="ts">
  /**
   * Generic derived form and confirmation sheet for any declared kind operation.
   * Adapts `experiments/drawing-spike/web/src/views/FormSheet.svelte` to revision 9 declarations.
   */
  import { Button, Sheet } from "@calternal/ui";
  import { humanize } from "./format";

  export interface FormFieldSpec {
    name: string;
    label: string;
    type: string;
    required?: boolean;
    value?: string;
  }

  let {
    title,
    submitLabel,
    fields = [],
    confirmText,
    danger = false,
    onsubmit,
    onclose,
  }: {
    title: string;
    submitLabel: string;
    fields?: FormFieldSpec[];
    confirmText?: string;
    danger?: boolean;
    onsubmit: (values: Record<string, string>) => Promise<void> | void;
    onclose: () => void;
  } = $props();

  const values = $state<Record<string, string>>({});
  let busy = $state(false);
  let error = $state("");

  $effect.pre(() => {
    for (const f of fields) {
      if (!(f.name in values)) values[f.name] = f.value ?? "";
    }
  });

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    error = "";
    try {
      await onsubmit({ ...values });
      onclose();
    } catch (err) {
      error = err instanceof Error ? err.message : "Operation failed.";
    } finally {
      busy = false;
    }
  }

  const narrow = typeof matchMedia !== "undefined" && matchMedia("(max-width: 719px)").matches;
</script>

<Sheet open label={title} {onclose} side={narrow ? "bottom" : "center"}>
  <form class="f" onsubmit={handleSubmit}>
    <h2>{title}</h2>
    {#if confirmText}
      <p class="confirm">{confirmText}</p>
    {/if}
    {#each fields as f (f.name)}
      <label>
        <span>{f.label || humanize(f.name)}{#if !f.required}<em> (optional)</em>{/if}</span>
        {#if f.type === "long text"}
          <textarea
            name={f.name}
            rows="3"
            bind:value={values[f.name]}
            required={f.required}
          ></textarea>
        {:else}
          <input
            name={f.name}
            type={f.type === "url" ? "url" : "text"}
            bind:value={values[f.name]}
            required={f.required}
          />
        {/if}
      </label>
    {/each}
    {#if error}
      <p class="err" role="alert">{error}</p>
    {/if}
    <div class="btns">
      <Button text="Cancel" variant="quiet" onclick={onclose} />
      <Button
        type="submit"
        text={submitLabel}
        variant={danger ? "destructive" : "primary"}
        {busy}
      />
    </div>
  </form>
</Sheet>

<style>
  .f { display: grid; gap: var(--s-5); padding: var(--s-6); min-width: min(26rem, 90vw); }
  h2 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); color: var(--ink); }
  .confirm { margin: 0; font-size: var(--text-ui); color: var(--ink-2); }
  label { display: grid; gap: var(--s-2); }
  label span { font-size: var(--text-label); color: var(--ink-2); }
  em { color: var(--ink-3); font-style: normal; }
  input, textarea {
    padding: var(--s-4);
    border: 0;
    border-radius: var(--r-select);
    background: rgb(var(--ink-rgb) / 0.07);
    color: var(--ink);
    font: inherit;
    font-size: var(--text-ui);
    resize: vertical;
  }
  input:focus-visible, textarea:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .err { margin: 0; color: var(--danger); font-size: var(--text-label); }
  .btns { display: flex; justify-content: flex-end; gap: var(--s-3); }
</style>
