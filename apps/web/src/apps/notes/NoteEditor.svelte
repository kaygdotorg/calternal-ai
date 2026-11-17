<script lang="ts">
  // Hosts the block editor (packages/editor/src/block, a lazy chunk) for one note. The editor knows nothing about Notes: this file hands it the note's Markdown, tells it which blocks are tasks,
  // draws the block menu with the design system's Menu, and reports edits. The text is not mirrored into component state (that would copy a long document per keystroke): `onedit` gets it
  // and the parent debounces saving. Opening another note replaces the document and forgets the history.
  import { Menu, type MenuItem } from "@calternal/ui";
  import MentionPicker from "../../palette/MentionPicker.svelte";
  import { blockPool, mentionPool, notePool, pick, tagPool, type Mention } from "../../palette/mentions";
  import { onMount } from "svelte";
  import type { BlockEditor, Block, Trigger } from "../../../../../packages/editor/src/block/index";
  let { id, text, onedit, onheadings, label = "Note", notes = [], threads = [], agents = [], tags = [] }: { id: string; text: string; onedit: (text: string) => void; onheadings?: (h: { level: number; text: string; from: number }[]) => void; label?: string; notes?: { id: string; title: string }[]; threads?: { id: string; title: string }[]; agents?: string[]; tags?: string[] } = $props();
  let host = $state<HTMLElement>();
  let ed: BlockEditor | undefined;
  let openedId = "";
  let menu = $state<{ block: Block; rect: DOMRect } | null>(null);
  const items: MenuItem[] = [{ id: "up", label: "Move up" }, { id: "down", label: "Move down" }, { id: "duplicate", label: "Duplicate" }, { id: "delete", label: "Delete", danger: true }];
  // ---- the pickers: @ (people, agents, threads, notes), [[ (notes), # (tags), / (insert menu), the same component as the composer ----
  let trig = $state<Trigger | null>(null);
  let sel = $state(0);
  let dismissed = $state<number | null>(null); // the trigger position the person closed with Escape
  const others = $derived(notes.filter((n) => n.id !== id));
  const groups = $derived.by(() => {
    if (!trig || trig.from === dismissed) return [];
    const pool = trig.kind === "mention" ? mentionPool(agents, threads, others) : trig.kind === "link" ? notePool(others) : trig.kind === "tag" ? tagPool(tags, trig.query) : blockPool();
    return pick(pool, trig.query, trig.kind === "mention" ? 4 : 8);
  });
  const flat = $derived(groups.flatMap((g) => g.entries));
  $effect(() => { flat; sel = 0; });
  const anchor = $derived(trig?.rect ? { left: trig.rect.left, top: trig.rect.top, right: trig.rect.right, bottom: trig.rect.bottom } : null);
  function choose(m: Mention) {
    if (!trig || !ed) return;
    const at = trig.from + (m.caret ?? m.insert.length);
    // A mention or a note link ends with a space so typing can go on; a block or a tag does not need one.
    const tail = m.kind === "person" || m.kind === "agent" || m.kind === "thread" || (m.kind === "item") ? " " : "";
    ed.apply({ from: trig.from, to: trig.to, insert: m.insert + tail }, "command");
    ed.view.dispatch({ selection: { anchor: m.caret !== undefined ? at : at + tail.length } });
    ed.focus();
    dismissed = trig.from; // a tag ends at the caret, so the same trigger would open again: it stays closed until the caret leaves it
    trig = null;
  }
  // While a picker is open it owns Up, Down, Enter, Tab and Escape.
  function pickerKey(e: KeyboardEvent): boolean {
    if (!flat.length) return false;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); sel = (sel + (e.key === "ArrowDown" ? 1 : flat.length - 1)) % flat.length; return true; }
    if (e.key === "Enter" || e.key === "Tab") { e.preventDefault(); choose(flat[sel]!); return true; }
    if (e.key === "Escape") { e.preventDefault(); dismissed = trig?.from ?? null; return true; }
    return false;
  }
  let timer = 0;
  const heads = () => { clearTimeout(timer); timer = window.setTimeout(() => ed && onheadings?.(ed.headings()), 250); };

  onMount(() => {
    let dead = false;
    void import("../../../../../packages/editor/src/block/index").then(({ createBlockEditor }) => {
      if (dead || !host) return;
      ed = createBlockEditor({
        parent: host, text, label,
        // A task is a list item that starts with a box: the host says so, the editor package has no such kind.
        kinds: (b, s) => (b.kind === "item" && /^\s*[-*+] \[[ xX]\]/.test(s.doc.sliceString(b.from, Math.min(b.to, b.from + 8))) ? "task" : null),
        onchange: (t, origin) => { if (origin !== "remote") onedit(t); heads(); },
        onMenu: (block, rect) => (menu = { block, rect }),
        onTrigger: (t) => { if (!t) dismissed = null; trig = t; },
        pickerKey,
      });
      openedId = id;
      onheadings?.(ed.headings());
    });
    return () => { dead = true; clearTimeout(timer); ed?.destroy(); ed = undefined; };
  });
  // Another note, or the same note saved elsewhere with different text: replace the document (never while it already shows exactly this text).
  $effect(() => {
    const t = text, i = id;
    if (!ed || (i === openedId && ed.value === t)) return;
    openedId = i;
    ed.setValue(t);
    onheadings?.(ed.headings());
  });
  export function scrollTo(pos: number) { ed?.scrollTo(pos); ed?.focus(); }
  export function setText(t: string) { ed?.setValue(t); }
  export function current(): string { return ed?.value ?? text; }
</script>

<div class="doc" bind:this={host}></div>
<MentionPicker {groups} bind:sel {anchor} prefer="bottom" label={trig?.kind === "link" ? "Link a note" : trig?.kind === "tag" ? "Tag" : trig?.kind === "skill" ? "Insert" : "Mention"} onchoose={choose} onclose={() => (dismissed = trig?.from ?? null)} />
<Menu open={!!menu} {items} label="Block actions" anchor={menu ? { left: menu.rect.left, top: menu.rect.bottom, right: menu.rect.right, bottom: menu.rect.bottom } : null} placement="bottom" onclose={() => (menu = null)} onpick={(a) => { if (menu && ed) ed.act(a as "up", menu.block); }} />

<style>
  .doc { position: relative; min-height: 40vh; padding-left: 0; }
  .doc :global(.cm-editor) { position: relative; }
</style>
