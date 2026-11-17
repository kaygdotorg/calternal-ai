<script lang="ts">
  import { FieldEditor } from "../../../editor/src/index";
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
    editor = $bindable(),
    marks,
    onselect,
    el = $bindable(),
  }: {
    /** Focus and select all on mount (rename fields). */ autoselect?: boolean;
    /** The editor behind the field (`packages/editor`): apply a change from code as one undo step (`editor.apply({from, to, insert})`), undo, redo. */ editor?: FieldEditor;
    /** Words to highlight behind the text, as ranges of `value` (the recognised words); drawn on a mirror layer, never in the field. */ marks?: { start: number; end: number; kind: string }[];
    /** Called when the caret or the selection moves (for pickers that follow the caret). */ onselect?: () => void;
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
  // The mirror: a hidden-text copy of the field behind it, in the same box, so the marked ranges show as soft highlights. Its metrics are copied from the field.
  let mirror = $state<HTMLElement>();
  const PROPS = ["font", "lineHeight", "letterSpacing", "padding", "textIndent", "wordSpacing", "textTransform", "textAlign"] as const;
  function syncMirror() {
    if (!mirror || !el) return;
    const cs = getComputedStyle(el);
    for (const p of PROPS) (mirror.style as unknown as Record<string, string>)[p] = (cs as unknown as Record<string, string>)[p]!;
    mirror.style.whiteSpace = multiline ? "pre-wrap" : "pre";
    mirror.style.overflowWrap = "break-word";
    mirror.scrollTop = el.scrollTop;
    mirror.scrollLeft = el.scrollLeft;
  }
  const parts = $derived.by(() => {
    if (!marks?.length) return [{ t: value, on: false, kind: "" }];
    const out: { t: string; on: boolean; kind: string }[] = [];
    let at = 0;
    for (const m of [...marks].sort((a, b) => a.start - b.start)) {
      if (m.start < at || m.end > value.length) continue;
      if (m.start > at) out.push({ t: value.slice(at, m.start), on: false, kind: "" });
      out.push({ t: value.slice(m.start, m.end), on: true, kind: m.kind });
      at = m.end;
    }
    if (at < value.length) out.push({ t: value.slice(at), on: false, kind: "" });
    return out;
  });
  $effect(() => { value; marks; queueMicrotask(syncMirror); });
  // The field editor (docs/editor-port-plan.md, stage 1): history of the person's own changes across typed and programmatic edits, the input-method gate and the keystroke probe.
  $effect(() => {
    if (!el) return;
    const fe = new FieldEditor(el);
    editor = fe;
    return () => { fe.destroy(); editor = undefined; };
  });
  $effect(() => { value; editor?.sync(); });
  $effect(() => { value; grow(); });
  $effect(() => { if (autoselect && el) { el.focus(); el.select(); } });
</script>

<div class="ie">
  {#if marks}<div class="mirror" aria-hidden="true" bind:this={mirror}>{#each parts as x}{#if x.on}<mark data-kind={x.kind}>{x.t}</mark>{:else}{x.t}{/if}{/each}{"\n"}</div>{/if}
  {#if multiline}
    <textarea {id} class={["inline-editor", klass]} bind:this={el} bind:value rows="1" aria-label={label} {placeholder} oninput={() => { grow(); oninput?.(); }} onkeydown={key} onkeyup={onselect} onclick={onselect} onscroll={syncMirror} {onblur}></textarea>
  {:else}
    <input {id} class={["inline-editor", klass]} bind:this={el} bind:value aria-label={label} {placeholder} autocomplete="off" spellcheck="false" oninput={() => oninput?.()} onkeydown={key} onkeyup={onselect} onclick={onselect} {onblur} />
  {/if}
</div>

<style>
  .ie { position: relative; display: grid; flex: 1; min-width: 0; }
  .ie > :global(.inline-editor) { grid-area: 1 / 1; position: relative; width: 100%; min-width: 0; }
  .mirror { grid-area: 1 / 1; position: absolute; inset: 0; overflow: hidden; color: transparent; pointer-events: none; box-sizing: border-box; }
  .mirror mark { background: var(--accent-soft); color: transparent; border-radius: var(--s-2); box-shadow: 0 0 0 var(--s-1) var(--accent-soft); }
  .mirror mark[data-kind="mention"], .mirror mark[data-kind="skill"] { background: color-mix(in oklab, var(--accent) 22%, transparent); box-shadow: 0 0 0 var(--s-1) color-mix(in oklab, var(--accent) 22%, transparent); }
  .inline-editor { font: inherit; color: var(--ink); caret-color: var(--accent); background: none; border: 0; min-width: 0; }
  textarea.inline-editor { resize: none; line-height: 1.5; }
  .inline-editor::placeholder { color: var(--ink-3); opacity: 1; }
  .inline-editor:focus, .inline-editor:focus-visible { outline: none; }
</style>
