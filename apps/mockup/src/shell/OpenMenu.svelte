<script lang="ts">
  // The context menu of anything that opens in a window (a strip app, a section's heading, a row, card, cell or event; lib/intent.ts says what): Open, and Open in new window; a strip app also gets Move left and Move right (the alternative to dragging it). Right-click, or the Menu key / Shift+F10 on a focused one. Not on a phone, where a second window is a pushed page.
  import { Menu, type MenuItem } from "@calternal/ui";
  import { target as find, openNew, type Target } from "../lib/intent";
  import { strip } from "../strip.svelte";
  let { narrow, onapp }: { narrow: boolean; onapp: (id: string) => void } = $props();
  let target = $state<Target | null>(null), at = $state<{ left: number; top: number; right: number; bottom: number } | null>(null);
  function context(e: MouseEvent) {
    if (narrow) return; const t = find(e.target as Element); if (!t) return;
    e.preventDefault(); e.stopPropagation(); const r = (e.target as Element).getBoundingClientRect(); const x = e.clientX || r.left, y = e.clientY || r.bottom;
    target = t; at = { left: x, top: y, right: x, bottom: y };
  }
  const items = $derived<MenuItem[]>(!target ? [] : [{ id: "open", label: "Open", icon: "expand" }, { id: "new", label: "Open in new window", icon: "external" }, ...(target.kind === "app" ? [{ id: "left", label: "Move left", icon: "back" as const, disabled: strip.ids()[0] === target.id }, { id: "right", label: "Move right", icon: "chevron" as const, disabled: strip.ids().at(-1) === target.id }] : [])]);
  function pick(id: string) {
    const t = target; target = null; if (!t) return;
    if (id === "left" || id === "right") return strip.move(t.id, id === "left" ? -1 : 1);
    if (id === "new") return openNew(t, onapp);
    // "Open" is the element's own click
    (t.el.matches("button, a") ? t.el : (t.el.querySelector<HTMLElement>("button, a") ?? t.el)).click();
  }
  // taken in the capture phase, before a component's own context menu (the app switcher, a file row)
  $effect(() => { addEventListener("contextmenu", context, true); return () => removeEventListener("contextmenu", context, true); });
</script>

<Menu open={target !== null} {items} label="Open" anchor={at} onpick={pick} onclose={() => (target = null)} trigger={target?.el ?? null} />
