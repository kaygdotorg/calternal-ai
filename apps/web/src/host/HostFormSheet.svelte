<script lang="ts">
  /**
   * Generic derived form and confirmation sheet for any declared kind operation.
   * Supports `text`, `long text`, `url`, `date`, `time`, `when`, `choice`, `yes/no`,
   * typed `list(...)` / `tags`, and validated `repeat` (RRULE) inputs.
   */
  import { Button, Sheet } from "@calternal/ui";
  import { humanize, validateRrule } from "./format";

  export interface FormFieldSpec {
    name: string;
    label: string;
    type: string;
    required?: boolean;
    choices?: string[];
    itemType?: string;
    value?: unknown;
  }

  let {
    title,
    submitLabel,
    fields = [],
    omitEmptyOptional = false,
    confirmText,
    danger = false,
    onsubmit,
    onclose,
  }: {
    title: string;
    submitLabel: string;
    fields?: FormFieldSpec[];
    /** When true (e.g. `add`), empty optional inputs are omitted so the host applies defaults. */
    omitEmptyOptional?: boolean;
    confirmText?: string;
    danger?: boolean;
    onsubmit: (values: Record<string, unknown>) => Promise<void> | void;
    onclose: () => void;
  } = $props();

  const textValues = $state<Record<string, string>>({});
  const boolValues = $state<Record<string, boolean>>({});
  let busy = $state(false);
  let error = $state("");

  function isListField(f: FormFieldSpec): boolean {
    return f.type === "tags" || f.type.startsWith("list(") || Boolean(f.itemType);
  }

  function isChoiceField(f: FormFieldSpec): boolean {
    return Boolean(f.choices?.length) || f.type.startsWith("choice(");
  }

  $effect.pre(() => {
    for (const f of fields) {
      if (f.type === "yes/no") {
        if (!(f.name in boolValues)) boolValues[f.name] = Boolean(f.value);
      } else if (!(f.name in textValues)) {
        if (Array.isArray(f.value)) {
          textValues[f.name] = f.value.map((v) => String(v)).join(", ");
        } else if (f.value !== undefined && f.value !== null) {
          textValues[f.name] = String(f.value);
        } else {
          textValues[f.name] = "";
        }
      }
    }
  });

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (busy) return;
    error = "";

    const out: Record<string, unknown> = {};
    for (const f of fields) {
      if (f.type === "yes/no") {
        const b = Boolean(boolValues[f.name]);
        if (!omitEmptyOptional || f.required || b) out[f.name] = b;
        continue;
      }
      const raw = (textValues[f.name] ?? "").trim();
      if (!raw) {
        if (f.required) {
          error = `${f.label || humanize(f.name)} is required.`;
          return;
        }
        if (!omitEmptyOptional) out[f.name] = isListField(f) ? [] : null;
        continue;
      }
      if (f.type === "repeat") {
        const err = validateRrule(raw);
        if (err) {
          error = err;
          return;
        }
        out[f.name] = raw.toUpperCase();
      } else if (isListField(f)) {
        const items = raw
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean);
        if (items.length || !omitEmptyOptional) out[f.name] = items;
      } else if (f.type === "number" || f.type.startsWith("number(")) {
        const n = Number(raw);
        if (Number.isNaN(n)) {
          error = `${f.label || humanize(f.name)} must be a number.`;
          return;
        }
        out[f.name] = n;
      } else {
        out[f.name] = raw;
      }
    }

    busy = true;
    try {
      await onsubmit(out);
      onclose();
    } catch (err) {
      error = err instanceof Error ? err.message : "Operation failed.";
    } finally {
      busy = false;
    }
  }

  const narrow = typeof matchMedia !== "undefined" && matchMedia("(max-width: 719px)").matches;
  const RRULE_PRESETS = [
    ["Daily", "FREQ=DAILY"],
    ["Weekly", "FREQ=WEEKLY"],
    ["Monthly", "FREQ=MONTHLY"],
    ["Yearly", "FREQ=YEARLY"],
  ] as const;
</script>

<Sheet open label={title} {onclose} side={narrow ? "bottom" : "center"}>
  <form class="f" onsubmit={handleSubmit}>
    <h2>{title}</h2>
    {#if confirmText}
      <p class="confirm">{confirmText}</p>
    {/if}
    {#each fields as f (f.name)}
      {#if f.type === "yes/no"}
        <label class="chk-row">
          <input type="checkbox" name={f.name} bind:checked={boolValues[f.name]} />
          <span>{f.label || humanize(f.name)}</span>
        </label>
      {:else}
        <label>
          <span>{f.label || humanize(f.name)}{#if !f.required}<em> (optional)</em>{/if}</span>
          {#if f.type === "long text"}
            <textarea
              name={f.name}
              rows="3"
              bind:value={textValues[f.name]}
              required={f.required}
            ></textarea>
          {:else if isChoiceField(f)}
            <select name={f.name} bind:value={textValues[f.name]} required={f.required}>
              {#if !f.required}<option value="">—</option>{/if}
              {#each f.choices ?? [] as choice (choice)}
                <option value={choice}>{humanize(choice)}</option>
              {/each}
            </select>
          {:else if f.type === "date"}
            <input
              name={f.name}
              type="date"
              bind:value={textValues[f.name]}
              required={f.required}
            />
          {:else if f.type === "time"}
            <input
              name={f.name}
              type="datetime-local"
              bind:value={textValues[f.name]}
              required={f.required}
            />
          {:else if f.type === "when"}
            <input
              name={f.name}
              type="date"
              bind:value={textValues[f.name]}
              required={f.required}
            />
          {:else if f.type === "repeat"}
            <input
              name={f.name}
              type="text"
              placeholder="FREQ=WEEKLY;INTERVAL=1"
              bind:value={textValues[f.name]}
              required={f.required}
            />
            <div class="presets">
              {#each RRULE_PRESETS as [lbl, rule] (rule)}
                <button type="button" class="pre" onclick={() => (textValues[f.name] = rule)}>{lbl}</button>
              {/each}
            </div>
          {:else if isListField(f)}
            <input
              name={f.name}
              type="text"
              placeholder="Comma-separated values"
              bind:value={textValues[f.name]}
              required={f.required}
            />
          {:else}
            <input
              name={f.name}
              type={f.type === "url" ? "url" : f.type === "number" ? "number" : "text"}
              bind:value={textValues[f.name]}
              required={f.required}
            />
          {/if}
        </label>
      {/if}
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
  .f { display: grid; gap: var(--s-5); padding: var(--s-6); min-width: min(26rem, 90vw); max-height: 85dvh; overflow-y: auto; }
  h2 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); color: var(--ink); }
  .confirm { margin: 0; font-size: var(--text-ui); color: var(--ink-2); }
  label { display: grid; gap: var(--s-2); }
  label.chk-row { display: flex; align-items: center; gap: var(--s-3); cursor: pointer; }
  label.chk-row input { width: 18px; height: 18px; accent-color: var(--accent); }
  label span { font-size: var(--text-label); color: var(--ink-2); }
  em { color: var(--ink-3); font-style: normal; }
  input:not([type="checkbox"]), textarea, select {
    padding: var(--s-4);
    border: 0;
    border-radius: var(--r-select);
    background: rgb(var(--ink-rgb) / 0.07);
    color: var(--ink);
    font: inherit;
    font-size: var(--text-ui);
    resize: vertical;
  }
  input:focus-visible, textarea:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .presets { display: flex; flex-wrap: wrap; gap: var(--s-2); margin-top: var(--s-1); }
  .pre { padding: var(--s-1) var(--s-3); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink-2); font-size: var(--text-label); cursor: pointer; }
  .pre:hover { background: var(--hover); color: var(--ink); }
  .err { margin: 0; color: var(--danger); font-size: var(--text-label); }
  .btns { display: flex; justify-content: flex-end; gap: var(--s-3); }
</style>
