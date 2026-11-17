/**
 * Block chrome: a handle in the left margin beside the block under the pointer. Press and release opens the block's menu (the host draws it; `onMenu`); press and drag moves the block
 * (one transaction, one undo step) with a drop line showing where it lands. The handle is one element moved around, not one per block, so a long note pays nothing.
 */
import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { blockAt, blocksIn, moveBlock, type Block, type Kinds } from "./blocks";
import { origin } from "./origin";

export interface ChromeOptions { kinds?: Kinds; onMenu?: (b: Block, rect: DOMRect) => void; label?: string }

export function blockChrome(o: ChromeOptions = {}) {
  return ViewPlugin.fromClass(class {
    handle = document.createElement("button");
    drop = document.createElement("div");
    over: Block | null = null;
    dragging = false;
    constructor(readonly view: EditorView) {
      this.handle.type = "button";
      this.handle.className = "cm-handle";
      this.handle.setAttribute("aria-label", o.label ?? "Block actions");
      this.handle.tabIndex = -1;
      this.handle.textContent = "⋮⋮";
      this.drop.className = "cm-drop";
      view.dom.append(this.handle, this.drop);
      this.handle.addEventListener("pointerdown", this.down);
      view.dom.addEventListener("pointermove", this.move);
      view.dom.addEventListener("pointerleave", () => { if (!this.dragging) this.handle.style.opacity = "0"; });
    }
    update(u: ViewUpdate) { if (u.docChanged) this.handle.style.opacity = "0"; }
    destroy() { this.handle.remove(); this.drop.remove(); this.view.dom.removeEventListener("pointermove", this.move); }
    move = (e: PointerEvent) => {
      if (this.dragging || e.pointerType === "touch") return;
      const pos = this.view.posAtCoords({ x: e.clientX, y: e.clientY }, false);
      if (pos == null) return;
      const b = blockAt(this.view.state, pos, o.kinds);
      if (!b) { this.handle.style.opacity = "0"; return; }
      this.over = b;
      const c = this.view.coordsAtPos(b.from);
      const host = this.view.dom.getBoundingClientRect();
      if (!c) return;
      this.handle.style.opacity = "1";
      this.handle.style.top = `${c.top - host.top}px`;
    };
    down = (e: PointerEvent) => {
      const b = this.over;
      if (!b) return;
      e.preventDefault();
      const x0 = e.clientX, y0 = e.clientY;
      this.handle.setPointerCapture(e.pointerId);
      let target: number | null = null;
      const onmove = (m: PointerEvent) => {
        if (!this.dragging && Math.hypot(m.clientX - x0, m.clientY - y0) < 6) return;
        this.dragging = true;
        const pos = this.view.posAtCoords({ x: this.view.dom.getBoundingClientRect().left + 40, y: m.clientY }, false);
        if (pos == null) return;
        const hit = blocksIn(this.view.state, pos, pos, o.kinds).at(-1);
        if (!hit) return;
        const c = this.view.coordsAtPos(hit.from), host = this.view.dom.getBoundingClientRect();
        const before = m.clientY < (c ? (c.top + c.bottom) / 2 : m.clientY);
        target = before ? hit.from : Math.min(this.view.state.doc.length, hit.to + 1);
        const y = before ? c?.top : this.view.coordsAtPos(Math.max(hit.from, hit.to))?.bottom;
        if (y != null) { this.drop.style.opacity = "1"; this.drop.style.top = `${y - host.top}px`; }
      };
      const up = () => {
        this.handle.removeEventListener("pointermove", onmove);
        this.handle.removeEventListener("pointerup", up);
        this.drop.style.opacity = "0";
        if (this.dragging && target != null) {
          const ch = moveBlock(this.view.state, b, target);
          if (ch) this.view.dispatch({ changes: ch, annotations: origin.of("command"), userEvent: "move" });
        } else o.onMenu?.(b, this.handle.getBoundingClientRect());
        this.dragging = false;
      };
      this.handle.addEventListener("pointermove", onmove);
      this.handle.addEventListener("pointerup", up);
    };
  });
}
