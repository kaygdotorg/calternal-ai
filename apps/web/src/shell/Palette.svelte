<script lang="ts">
  // The palette: one field for search, go to, commands and "ask an agent". Opens with Cmd/Ctrl-K or the search button: a glass panel under the top edge on a
  // desktop, a bottom sheet on a phone. Plain language is read on the device (palette/engine.ts, no model): recognised words are highlighted behind the
  // text, one line says what Enter will do, Up and Down choose another row, Esc closes. Outward actions (Ask) are their own row and never the default.
  import { Icon, InlineEditor, Sheet, toast } from "@calternal/ui";
  import { marksOf, plan, setItems, type Row } from "../palette/engine";
  import { fixtureItems } from "../palette/fixtures";
  import { spaces } from "./spaces.svelte";
  import { navigate, segments } from "../lib/route.svelte";
  import { client, startThread, store } from "../apps/threads/store.svelte";
  import { shell } from "./shell.svelte";
  import { serverMode } from "../server/client";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  let text = $state("");
  let sel = $state(0);
  let list = $state<HTMLElement>();
  const provider = $derived(store.providers[0]?.name ?? "the agent");
  $effect(() => {
    if (!open) return;
    text = "";
    sel = 0;
    // The field takes focus in the same turn the dialog appears, so the first key after Ctrl+K lands in it. (It used to wait a frame, during which keys went to whatever had focus before.)
    // The sheet keeps a focused field (its own focus step skips when focus is already inside); one frame later is the fallback if the field was not focusable yet.
    const f = () => document.querySelector<HTMLElement>(".pal-input");
    f()?.focus();
    requestAnimationFrame(() => { if (document.activeElement !== f()) f()?.focus(); });
    const threads = store.threads.map((t) => ({ id: t.id, kind: "thread" as const, title: t.title, space: spaces.list.find((s) => s.id === t.spaceId)?.name ?? spaces.current.name, go: `/threads/${t.id}` }));
    if (!serverMode()) { setItems([...threads, ...fixtureItems()], spaces.current.name); return; } // the palette's data is fixtures, as is everything else, unless the real server is the source
    // The real server: this space's threads and notes (the newest page, and whatever the person types is looked up by title below), and every space the person is in, so none is unreachable from here.
    void import("../apps/notes/store.svelte").then(({ notes }) => { listed = notes.list.map((n) => ({ id: n.id, kind: "note" as const, title: n.title, space: spaces.current.name, go: `/notes/${n.id}` })); rebuild(threads); });
    rebuild(threads);
  });
  let listed = $state<{ id: string; kind: "note"; title: string; space: string; go: string }[]>([]);
  let found = $state<typeof listed>([]);
  const spaceItems = () => spaces.list.map((s) => ({ id: `space:${s.id}`, kind: "command" as const, title: `Space: ${s.name}`, space: spaces.current.name, op: `space.switch:${s.id}`, group: s.id === spaces.current.id ? "This space" : "Switch space", words: ["switch", "space", "change"] }));
  function rebuild(threads: Parameters<typeof setItems>[0]) {
    const seen = new Set(listed.map((n) => n.id));
    setItems([...threads, ...listed, ...found.filter((n) => !seen.has(n.id)), ...spaceItems()], spaces.current.name);
  }
  // Notes the newest page does not hold: while the person types, the space's title index is asked (one request per pause), so every note of the space is findable from here.
  $effect(() => {
    const q = text.trim();
    if (!open || !serverMode() || q.length < 2) { found = []; return; }
    const space = spaces.current;
    const h = setTimeout(async () => {
      try { const { notes } = await import("../apps/notes/store.svelte"); found = (await notes.client.lookupNotes(space.id, q)).map((n) => ({ id: n.id, kind: "note" as const, title: n.title, space: space.name, go: `/notes/${n.id}` })); rebuild(store.threads.map((t) => ({ id: t.id, kind: "thread" as const, title: t.title, space: spaces.list.find((s) => s.id === t.spaceId)?.name ?? space.name, go: `/threads/${t.id}` }))); } catch { found = []; }
    }, 150);
    return () => clearTimeout(h);
  });
  const p = $derived(plan(text, provider));
  $effect(() => { p; sel = 0; });
  const marks = $derived(marksOf(text));
  function run(r: Row | undefined) {
    if (!r) return;
    const a = r.action;
    onclose();
    if (a.op?.startsWith("space.switch:")) { spaces.switchTo(a.op.slice(13)); navigate("/threads"); }
    else if (a.go) navigate(a.go);
    else if (a.op === "help.shortcuts") shell.helpOpen = true;
    else if (a.op === "thread.new") void startThread().then((t) => navigate(`/threads/${t.id}`)).catch((e: Error) => toast(`Could not start a thread: ${e.message}`));
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
      <InlineEditor class="pal-input" label="Search, go to, or ask" bind:value={text} placeholder="Search, go to, or ask…" autoselect onsubmit={() => run(p.rows[sel])} onkeydown={key} {marks} />
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
