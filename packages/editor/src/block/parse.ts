/**
 * Parse ahead: before a frame is drawn, the document is parsed through the end of what is on screen (plus a margin), so the drawn decorations never flash raw syntax; the rest of the document is
 * parsed when the browser is idle. Without it a long note parses lazily as the person scrolls and the drawn lines jump. Cost: bounded by the viewport, and never more than `BUDGET_MS`
 * of work per frame; the idle parse yields between chunks.
 */
import { ensureSyntaxTree, syntaxTreeAvailable } from "@codemirror/language";
import { ViewPlugin, type EditorView, type ViewUpdate } from "@codemirror/view";

const BUDGET_MS = 12;
const MARGIN = 4000;

/** A view plugin that keeps the Markdown parse ahead of the viewport in idle time, so scrolling a long note finds the tree already built. Stops when the parse is complete or the view is destroyed. */
export const parseAhead = ViewPlugin.fromClass(class {
  idle = 0;
  constructor(readonly view: EditorView) { this.ahead(); this.later(); }
  update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged) { this.ahead(); this.later(); } }
  ahead() {
    const end = Math.min(this.view.state.doc.length, this.view.viewport.to + MARGIN);
    if (!syntaxTreeAvailable(this.view.state, end)) ensureSyntaxTree(this.view.state, end, BUDGET_MS);
  }
  later() {
    const ric = (globalThis as { requestIdleCallback?: (f: (d: { timeRemaining(): number }) => void) => number }).requestIdleCallback;
    if (!ric || this.idle) return;
    const step = (d: { timeRemaining(): number }) => {
      this.idle = 0;
      const doc = this.view.state.doc.length;
      if (syntaxTreeAvailable(this.view.state, doc)) return;
      ensureSyntaxTree(this.view.state, doc, Math.max(4, Math.min(20, d.timeRemaining())));
      if (!syntaxTreeAvailable(this.view.state, doc)) this.idle = ric(step);
    };
    this.idle = ric(step);
  }
  destroy() { const cic = (globalThis as { cancelIdleCallback?: (n: number) => void }).cancelIdleCallback; if (this.idle) cic?.(this.idle); }
});
