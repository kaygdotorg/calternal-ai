<script lang="ts">
  // Hosts the block editor (packages/editor/src/block, a lazy chunk) for one note. The editor knows nothing about Notes: this file hands it the note's Markdown, tells it which blocks are tasks,
  // draws the block menu with the design system's Menu, and reports edits. The text is not mirrored into component state (that would copy a long document per keystroke): `onedit` gets it
  // and the parent debounces saving. Opening another note replaces the document and forgets the history.
  import { Menu, type MenuItem } from "@calternal/ui";
  import { onMount } from "svelte";
  import type { BlockEditor, Block } from "../../../../../packages/editor/src/block/index";
  let { id, text, onedit, onheadings, label = "Note" }: { id: string; text: string; onedit: (text: string) => void; onheadings?: (h: { level: number; text: string; from: number }[]) => void; label?: string } = $props();
  let host = $state<HTMLElement>();
  let ed: BlockEditor | undefined;
  let openedId = "";
  let menu = $state<{ block: Block; rect: DOMRect } | null>(null);
  const items: MenuItem[] = [{ id: "up", label: "Move up" }, { id: "down", label: "Move down" }, { id: "duplicate", label: "Duplicate" }, { id: "delete", label: "Delete", danger: true }];
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
<Menu open={!!menu} {items} label="Block actions" anchor={menu ? { left: menu.rect.left, top: menu.rect.bottom, right: menu.rect.right, bottom: menu.rect.bottom } : null} placement="bottom" onclose={() => (menu = null)} onpick={(a) => { if (menu && ed) ed.act(a as "up", menu.block); }} />

<style>
  .doc { position: relative; min-height: 40vh; padding-left: 0; }
  .doc :global(.cm-editor) { position: relative; }
</style>
