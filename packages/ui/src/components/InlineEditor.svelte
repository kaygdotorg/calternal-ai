<script lang="ts">
  // The one text-input surface for typing inline: the composer, side chat, rename fields. It is a thin contract (value, submit, cancel, focus)
  // over a native field so that the Notes block editor's input can replace it by changing this file alone; callers never touch the DOM node
  // except through `focus()`. Enter submits (Shift+Enter is a new line in `multiline`), Escape cancels, IME composition is never interrupted.
  // Cost: one `input` handler, one layout read per keystroke for auto-grow (multiline only), no re-render of siblings.
  let {
    value = $bindable(""),
    multiline = false,
    label,
    placeholder = "",
    maxRows = 8,
    id,
    class: klass,
    onsubmit,
    oncancel,
    onblur,
    oninput,
    onkeydown,
    autoselect = false,
    el = $bindable(),
  }: {
    /** Focus and select all on mount (rename fields). */ autoselect?: boolean;
    value?: string; multiline?: boolean; label?: string; placeholder?: string; maxRows?: number; id?: string; class?: string;
    onsubmit?: (value: string) => void; oncancel?: () => void; onblur?: () => void; oninput?: () => void; onkeydown?: (e: KeyboardEvent) => void;
    el?: HTMLInputElement | HTMLTextAreaElement;
  } = $props();

  function grow() {
    if (!multiline || !el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, maxRows * 1.5 * parseFloat(getComputedStyle(el).fontSize))}px`;
  }
  function key(e: KeyboardEvent) {
    onkeydown?.(e);
    if (e.defaultPrevented || e.isComposing) return;
    if (e.key === "Enter" && !(multiline && e.shiftKey)) { e.preventDefault(); onsubmit?.(value); }
    else if (e.key === "Escape" && oncancel) { e.preventDefault(); oncancel(); }
  }
  $effect(() => { value; grow(); });
  $effect(() => { if (autoselect && el) { el.focus(); el.select(); } });
</script>

{#if multiline}
  <textarea {id} class={["inline-editor", klass]} bind:this={el} bind:value rows="1" aria-label={label} {placeholder} oninput={() => { grow(); oninput?.(); }} onkeydown={key} {onblur}></textarea>
{:else}
  <input {id} class={["inline-editor", klass]} bind:this={el} bind:value aria-label={label} {placeholder} autocomplete="off" spellcheck="false" oninput={() => oninput?.()} onkeydown={key} {onblur} />
{/if}

<style>
  .inline-editor { font: inherit; color: var(--ink); caret-color: var(--accent); background: none; border: 0; min-width: 0; }
  textarea.inline-editor { resize: none; line-height: 1.5; }
  .inline-editor::placeholder { color: var(--ink-3); opacity: 1; }
  .inline-editor:focus, .inline-editor:focus-visible { outline: none; }
</style>
