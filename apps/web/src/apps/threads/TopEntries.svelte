<script lang="ts">
  // The fixed top section of the Threads sidebar (top-entries.svelte.ts): New thread, Automations, Skills by default; the person adds, removes, renames and reorders entries.
  // Each entry is a row; pointing at it shows one more button that opens its menu (right-click and the menu key open it too): Rename, Move up, Move down, Remove. Alt+Up and Alt+Down reorder from the keyboard.
  // "Add" at the end of the section lists what can be added (Notes, Settings, a thread) and restores the defaults.
  import { Icon, IconButton, InlineEditor, Menu, Row, toast, type MenuItem } from "@calternal/ui";
  import { href, navigate, segments } from "../../lib/route.svelte";
  import { iconOf, top, type TopEntry } from "./top-entries.svelte";
  import { startThread, store } from "./store.svelte";
  import { spaces } from "../../shell/spaces.svelte";

  let editing = $state<string | null>(null);
  let draft = $state("");
  let said = $state("");
  let ctx = $state<{ x: number; y: number; entry?: TopEntry; add?: boolean } | null>(null);

  const here = $derived("/" + segments().join("/"));
  const pathOf = (e: TopEntry) => (e.target.startsWith("route:") ? e.target.slice(6) : e.target.startsWith("thread:") ? `/threads/${e.target.slice(7)}` : null);
  const selected = (e: TopEntry) => { const p = pathOf(e); return p !== null && here === p; };

  async function open(e: TopEntry) {
    if (e.target === "new-thread") {
      try { const t = await startThread(); navigate(`/threads/${t.id}`); } catch (err) { toast(`Could not start a thread: ${(err as Error).message}`); }
    }
  }

  const entryItems: MenuItem[] = [{ id: "rename", label: "Rename" }, { id: "up", label: "Move up" }, { id: "down", label: "Move down" }, { id: "remove", label: "Remove from this list", danger: true }];
  const addItems = $derived<MenuItem[]>([
    ...[["route:/notes", "Notes"], ["route:/settings", "Settings"]].filter(([t]) => !top.entries.some((e) => e.target === t)).map(([t, l]) => ({ id: `add:${t}`, label: l! })),
    ...store.threads.filter((t) => t.spaceId === spaces.current.id && !top.entries.some((e) => e.target === `thread:${t.id}`)).slice(0, 8).map((t) => ({ id: `add:thread:${t.id}`, label: `Thread: ${t.title}` })),
    { id: "reset", label: "Restore New thread, Automations, Skills" },
  ]);

  function pick(id: string) {
    const c = ctx; ctx = null;
    if (!c) return;
    if (c.entry) {
      const e = c.entry;
      if (id === "rename") { editing = e.id; draft = e.label; }
      else if (id === "up") top.move(e.id, -1);
      else if (id === "down") top.move(e.id, 1);
      else if (id === "remove") top.remove(e.id);
    } else if (id === "reset") top.reset();
    else if (id.startsWith("add:")) {
      const target = id.slice(4);
      const label = target.startsWith("thread:") ? (store.get(target.slice(7))?.title ?? "Thread") : target === "route:/notes" ? "Notes" : "Settings";
      top.add(target, label);
    }
  }
  function commit() { if (editing) top.rename(editing, draft); editing = null; }
  function key(ev: KeyboardEvent, e: TopEntry, i: number) {
    if (ev.key === "F2") { ev.preventDefault(); editing = e.id; draft = e.label; return; }
    if (ev.key === "ContextMenu" || (ev.shiftKey && ev.key === "F10")) { const r = (ev.currentTarget as HTMLElement).getBoundingClientRect(); ev.preventDefault(); ctx = { x: r.left + 24, y: r.bottom, entry: e }; return; }
    if (!ev.altKey || (ev.key !== "ArrowUp" && ev.key !== "ArrowDown")) return;
    ev.preventDefault();
    const by = ev.key === "ArrowUp" ? -1 : 1;
    top.move(e.id, by);
    said = `${e.label} moved to position ${Math.max(1, Math.min(top.entries.length, i + 1 + by))} of ${top.entries.length}`;
    queueMicrotask(() => document.querySelector<HTMLElement>(`[data-top="${e.id}"] a, [data-top="${e.id}"] button.row`)?.focus());
  }
</script>

<ul class="top" aria-label="Shortcuts">
  {#each top.entries as e, i (e.id)}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <li data-top={e.id} data-target={e.target} oncontextmenu={(ev) => { ev.preventDefault(); ctx = { x: ev.clientX, y: ev.clientY, entry: e }; }} onkeydown={(ev) => key(ev, e, i)}>
      {#if editing === e.id}
        <InlineEditor class="rename" label="Rename entry" bind:value={draft} autoselect onblur={commit} onsubmit={commit} oncancel={() => (editing = null)} />
      {:else}
        <div class="rw">
          {#if pathOf(e)}
            <Row title={e.label} href={href(pathOf(e)!)} selected={selected(e)}>{#snippet leading()}<span class="ic"><Icon inline name={iconOf(e.target)} /></span>{/snippet}</Row>
          {:else}
            <Row title={e.label} onclick={() => void open(e)}>{#snippet leading()}<span class="ic"><Icon inline name={iconOf(e.target)} /></span>{/snippet}</Row>
          {/if}
          <span class="acts"><IconButton small icon="more" label="Options: {e.label}" aria-haspopup="menu" onclick={(ev) => { const r = (ev.currentTarget as HTMLElement).getBoundingClientRect(); ctx = { x: r.right, y: r.bottom, entry: e }; }} /></span>
        </div>
      {/if}
    </li>
  {/each}
  <!-- The same row as the shortcuts above (icon column, indent, size and weight), so it reads as the last item of the list, not as something else. -->
  <li><Row title="Add to this list" onclick={(ev) => { const r = (ev.currentTarget as HTMLElement).getBoundingClientRect(); ctx = { x: r.left, y: r.bottom, add: true }; }}>{#snippet leading()}<span class="ic"><Icon inline name="plus" /></span>{/snippet}</Row></li>
</ul>
<p class="visually-hidden" role="status" aria-live="polite">{said}</p>

<Menu open={!!ctx} items={ctx?.add ? addItems : entryItems} label={ctx?.add ? "Add to the shortcuts" : "Shortcut"} anchor={ctx ? { left: ctx.x, top: ctx.y, right: ctx.x, bottom: ctx.y } : null} placement="bottom" onclose={() => (ctx = null)} onpick={pick} />

<style>
  .top { display: grid; gap: var(--s-1); }
  li { position: relative; }
  .rw { position: relative; }
  .ic { flex: none; display: grid; color: var(--ink-2); }
  .acts { position: absolute; right: var(--s-2); top: 50%; translate: 0 -50%; opacity: 0; pointer-events: none; transition: opacity var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .rw:hover .acts, .rw:focus-within .acts { opacity: 1; pointer-events: auto; } }
</style>
