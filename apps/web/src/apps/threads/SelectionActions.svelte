<script lang="ts">
  // Two actions on selected chat text, in a small glass toolbar above the selection:
  //   Comment          puts the selection in the composer as a Markdown quote, a blank line, then the caret (compose.svelte.ts)
  //   Ask in Side Chat opens the right sidebar's chat with the selection quoted
  // It appears for a mouse, touch or keyboard selection made inside `root` (message text only: the rest of the page is not selectable, base.css) and
  // disappears when the selection collapses, the list scrolls or Escape is pressed. The buttons keep the selection alive (a press must not clear it).
  import { Toolbar, Pill } from "@calternal/ui";
  let { root, oncomment, onask }: { root: HTMLElement | undefined; oncomment: (text: string) => void; onask: (text: string) => void } = $props();
  let at = $state<{ x: number; y: number; text: string } | null>(null);
  let frame = 0;

  function read() {
    frame = 0;
    const sel = getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount || !root) return void (at = null);
    const range = sel.getRangeAt(0);
    const host = range.commonAncestorContainer.nodeType === 1 ? (range.commonAncestorContainer as Element) : range.commonAncestorContainer.parentElement;
    const text = sel.toString().trim();
    if (!host || !root.contains(host) || !host.closest(".selectable") || !text) return void (at = null);
    const r = range.getClientRects()[0] ?? range.getBoundingClientRect();
    at = { x: Math.min(Math.max(r.left + r.width / 2, 120), innerWidth - 120), y: Math.max(r.top, 56), text };
  }
  const schedule = () => { if (!frame) frame = requestAnimationFrame(read); };
  $effect(() => {
    document.addEventListener("selectionchange", schedule);
    const hide = () => (at = null);
    root?.addEventListener("scroll", hide, { capture: true, passive: true });
    const esc = (e: KeyboardEvent) => e.key === "Escape" && (at = null);
    addEventListener("keydown", esc);
    return () => { document.removeEventListener("selectionchange", schedule); root?.removeEventListener("scroll", hide, true); removeEventListener("keydown", esc); cancelAnimationFrame(frame); };
  });
  function act(f: (t: string) => void) {
    if (!at) return;
    const t = at.text;
    getSelection()?.removeAllRanges();
    at = null;
    f(t);
  }
</script>

{#if at}
  <div class="sel" style:left="{at.x}px" style:top="{at.y}px" onmousedown={(e) => e.preventDefault()} role="presentation">
    <Toolbar label="Selection actions">
      <Pill label="Comment" icon="quote" showLabel glass={false} onclick={() => act(oncomment)} />
      <Pill label="Ask in Side Chat" icon="threads" showLabel glass={false} onclick={() => act(onask)} />
    </Toolbar>
  </div>
{/if}

<style>
  .sel { position: fixed; z-index: 20; translate: -50% calc(-100% - var(--s-3)); }
</style>
