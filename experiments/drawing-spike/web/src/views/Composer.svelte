<script lang="ts">
  // The generic composer: draws a part's `composer` declaration in a slot. Its send operation's inputs are bound by the part
  // (`inputs`: this, draft, quote); the text field is the draft, kept on the device at once and stored as `draft.<item>`
  // (state.set, buffered by the host); a quote picked by a record's compose action shows above the field and is cleared on
  // send. Send is one call; the composer never reads the record file.
  import { Icon } from "@calternal/ui";
  import { opOf, type Json, type Row } from "../lib/model";
  import { call } from "../lib/calls.svelte";
  let { part, row, quoting = $bindable() }: { part: Json; row: Row; quoting: { anchor: string; author: string; excerpt: string } | null } = $props();
  const c = $derived(part.composer);
  const op = $derived(opOf(c.send) ?? {});
  const key = $derived(`draft.${row.item}`);
  // the field's hint is the label of the input the draft is bound to; a bare `text()` has none, so a neutral word stands in
  const hint = $derived(op.input?.[Object.entries<string>(c.inputs ?? {}).find(([, s]) => s === "draft")?.[0] ?? ""]?.label ?? "Write…");
  let draft = $state("");
  $effect.pre(() => { draft = c.draft ? (localStorage.getItem(key) ?? "") : ""; });
  const save = () => { if (c.draft) localStorage.setItem(key, draft); };
  const source = (src: string) => (src === "this" ? row.item : src === "draft" ? draft : src === "quote" ? (quoting?.anchor ?? null) : src);
  function send(e: Event) {
    e.preventDefault(); if (!draft.trim()) return;
    call(c.send, Object.fromEntries(Object.entries<string>(c.inputs ?? {}).map(([k, src]) => [k, source(src)]).filter(([, v]) => v != null)));
    draft = ""; quoting = null; save();
  }
</script>

<form class="cp" onsubmit={send} aria-label="Composer">
  {#if quoting}<div class="q"><span class="qa">{quoting.author}</span><span class="qx">{quoting.excerpt}</span><button type="button" class="x" aria-label="Remove quote" onclick={() => (quoting = null)}><Icon name="close" inline /></button></div>{/if}
  <div class="row">
    <textarea rows="1" bind:value={draft} oninput={save} placeholder={hint} aria-label={hint}
      onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) send(e); }}></textarea>
    <button type="submit" class="send" disabled={!draft.trim()} aria-label={op.title ?? "Send"}><Icon name="send" inline /></button>
  </div>
</form>

<style>
  .cp { flex: none; display: grid; gap: var(--s-3); margin: 0 var(--s-5) var(--s-5); padding: var(--s-3); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); }
  .q { position: relative; display: grid; gap: 2px; padding: var(--s-2) var(--s-8) var(--s-2) calc(var(--s-4) + 3px); background: color-mix(in oklab, var(--accent) 12%, transparent); border-radius: 0 var(--r-select) var(--r-select) 0; } .q::before { content: ""; position: absolute; inset-block: 0; left: 0; width: 3px; background: var(--accent); }
  .qa { color: var(--accent); font-weight: var(--strong-weight); font-size: var(--text-label); } .qx { color: var(--ink-2); font-size: var(--text-label); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x { position: absolute; right: var(--s-2); top: 50%; translate: 0 -50%; width: 22px; height: 22px; display: grid; place-items: center; border-radius: 50%; }
  .row { display: flex; align-items: flex-end; gap: var(--s-3); }
  textarea { flex: 1; min-width: 0; min-height: 34px; max-height: 8rem; padding: var(--s-4) var(--s-3); border: 0; outline: 0; resize: none; background: none; color: var(--ink); font: inherit; field-sizing: content; }
  .send { flex: none; display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; background: var(--accent-base); color: #fff; } .send:disabled { opacity: 0.4; }
  @media (max-width: 719px) { .cp { margin: 0 var(--s-4) var(--s-4); } }
</style>
