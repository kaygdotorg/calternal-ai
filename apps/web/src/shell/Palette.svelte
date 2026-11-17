<script lang="ts">
  // The palette: one field for search, go to, commands and "ask an agent". Opens with Cmd/Ctrl-K or the search button: a glass panel under the top edge on a
  // desktop, a bottom sheet on a phone. Plain language is read on the device (palette/engine.ts, no model): recognised words are highlighted behind the
  // text, one line says what Enter will do, Up and Down choose another row, Esc closes. Outward actions (Ask) are their own row and never the default.
  import { Icon, InlineEditor, Sheet, toast } from "@calternal/ui";
  import { plan, setItems, type Row } from "../palette/engine";
  import { fixtureItems, THIS_SPACE } from "../palette/fixtures";
  import { navigate, segments } from "../lib/route.svelte";
  import { client, store } from "../apps/threads/store.svelte";
  import { shell } from "./shell.svelte";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  let text = $state("");
  let sel = $state(0);
  let list = $state<HTMLElement>();
  const provider = $derived(store.providers[0]?.name ?? "the agent");
  $effect(() => {
    if (!open) return;
    text = "";
    sel = 0;
    // The sheet focuses its dialog when it opens; the field takes focus right after.
    requestAnimationFrame(() => document.querySelector<HTMLElement>(".pal-input")?.focus());
    setItems([...store.threads.map((t) => ({ id: t.id, kind: "thread" as const, title: t.title, space: THIS_SPACE, go: `/threads/${t.id}` })), ...fixtureItems()], THIS_SPACE);
  });
  const p = $derived(plan(text, provider));
  $effect(() => { p; sel = 0; });
  // The highlight: the text cut at the recognised spans (kept as text, never claimed twice).
  const parts = $derived.by(() => {
    const out: { t: string; on: boolean }[] = [];
    let at = 0;
    for (const s of p.parsed.spans) {
      if (s.start < at) continue;
      if (s.start > at) out.push({ t: text.slice(at, s.start), on: false });
      out.push({ t: text.slice(s.start, s.end), on: true });
      at = s.end;
    }
    if (at < text.length) out.push({ t: text.slice(at), on: false });
    return out;
  });
  function run(r: Row | undefined) {
    if (!r) return;
    const a = r.action;
    onclose();
    if (a.go) navigate(a.go);
    else if (a.op === "help.shortcuts") shell.helpOpen = true;
    else if (a.ask) {
      const s = segments();
      if (s[0] === "threads" && s[1]) { void client.send(s[1], a.ask, crypto.randomUUID()); toast("Sent to the agent"); }
      else toast("Open a thread to ask in it");
    } else toast("Not available in this preview");
  }
  function key(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const n = p.rows.length;
      if (n) sel = (sel + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      queueMicrotask(() => list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" }));
    }
  }
</script>

<Sheet {open} label="Search and commands" {onclose} side={shell.narrow ? "bottom" : "top"}>
  <div class="in">
    <div class="field">
      <Icon inline name="search" />
      <div class="ed">
        <div class="mirror" aria-hidden="true">{#each parts as x}{#if x.on}<mark>{x.t}</mark>{:else}{x.t}{/if}{/each}</div>
        <InlineEditor class="pal-input" label="Search, go to, or ask" bind:value={text} placeholder="Search, go to, or ask…" autoselect onsubmit={() => run(p.rows[sel])} onkeydown={key} />
      </div>
    </div>
    {#if p.intent}<p class="intent" aria-live="polite">{p.intent}</p>{/if}
    <ul bind:this={list} role="listbox" aria-label="Results">
      {#each p.rows as r, i (r.id + i)}
        <li role="option" id="pal-{i}" aria-selected={i === sel} class:on={i === sel} class="r {r.kind}" onpointermove={() => (sel = i)} onclick={() => run(r)} onkeydown={() => {}}>
          <span class="l">{r.label}</span>
          {#if r.space}<span class="sp">{r.space}</span>{/if}
          <span class="g">{r.sub}</span>
        </li>
      {/each}
    </ul>
    {#if !text.trim()}<p class="hint">Type to search every space, go somewhere, run a command, or ask an agent. <kbd>Esc</kbd> closes.</p>{/if}
  </div>
</Sheet>

<style>
  .in { display: grid; gap: var(--s-3); padding: var(--s-5); }
  .field { display: flex; align-items: center; gap: var(--s-4); min-height: var(--pill-h); padding: 0 var(--s-6); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-3); }
  .ed { position: relative; flex: 1; min-width: 0; font-size: var(--text-ui); }
  .mirror { position: absolute; inset: 0; display: flex; align-items: center; white-space: pre; overflow: hidden; color: transparent; pointer-events: none; }
  .mirror mark { background: var(--accent-soft); border-radius: var(--s-2); color: transparent; box-shadow: 0 0 0 var(--s-1) var(--accent-soft); }
  :global(.pal-input) { position: relative; width: 100%; height: var(--pill-h); }
  .intent { padding: 0 var(--s-4); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-2); }
  ul { display: grid; gap: var(--s-1); max-height: min(50vh, 24rem); overflow-y: auto; scrollbar-width: none; }
  .r { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); cursor: pointer; font-size: var(--text-ui); }
  .r.on { background: var(--accent-soft); }
  .r.ask { color: var(--ink-2); }
  .l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sp { font-size: var(--text-label); padding: 0 var(--s-3); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); }
  .g { font-size: var(--text-label); color: var(--ink-3); }
  .hint { padding: var(--s-3) var(--s-4); font-size: var(--text-label); color: var(--ink-3); }
  kbd { font: inherit; padding: 0 var(--s-2); border-radius: var(--s-2); background: rgb(var(--ink-rgb) / 0.08); }
</style>
