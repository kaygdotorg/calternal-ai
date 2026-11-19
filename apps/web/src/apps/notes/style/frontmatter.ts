/**
 * Reading a note's settings out of its frontmatter while the editor runs. The text is the only record of a note's style (the `calternal-*` lines of its frontmatter), so this reads the block in the
 * document and nothing else. `settingsWatch` is an editor extension the host adds through `createBlockEditor({ extensions })`: it reports the pairs when the editor is built and afterwards only when a
 * change touches the block. Cost per keystroke in the body: one `touchesRange` test against the block's last known end (three characters when the note has none); nothing is parsed and nothing drawn.
 * The block rule is the editor's: it starts on line 1 with `---`, ends at the next `---` line and must lie within the first 64 KB.
 */
import { type EditorState, type Extension, type Text } from "@codemirror/state";
import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

const LIMIT = 64 * 1024;

/** Where the body starts: just after the closing `---` line break, or 0 when the note has no frontmatter. */
export function blockEnd(doc: Text): number {
  if (doc.lines < 3 || !/^---\r?$/.test(doc.line(1).text)) return 0;
  for (let i = 3; i <= doc.lines; i++) {
    const l = doc.line(i);
    if (l.to + 1 > LIMIT) return 0;
    if (/^---[ \t]*\r?$/.test(l.text)) return l.to + 1;
  }
  return 0;
}

/** The note's frontmatter as `key: value` pairs, values unquoted. Lines that are not pairs are skipped. */
export function pairs(state: EditorState): Record<string, string> {
  const doc = state.doc, end = blockEnd(doc), out: Record<string, string> = {};
  if (!end) return out;
  const last = doc.lineAt(Math.min(end - 1, doc.length)).number - 1;
  for (let i = 2; i <= last; i++) {
    const m = /^([A-Za-z][\w-]*):[ \t]*(.*?)[ \t]*\r?$/.exec(doc.line(i).text);
    if (m) out[m[1]!] = m[2]!.replace(/^(["'])(.*)\1$/, "$2");
  }
  return out;
}

/** The extension: calls `report` with the pairs at build and whenever a change touches the frontmatter block (or creates or removes it). */
export function settingsWatch(report: (fm: Record<string, string>) => void): Extension {
  return ViewPlugin.fromClass(class {
    end: number;
    constructor(view: EditorView) { this.end = blockEnd(view.state.doc); report(pairs(view.state)); }
    update(u: ViewUpdate) {
      if (!u.docChanged) return;
      // With no block, only a change at the very top can make one; with one, only a change before its end can alter it. Anything else (the body) stops here.
      if (!u.changes.touchesRange(0, this.end ? this.end : 4)) return;
      this.end = blockEnd(u.state.doc);
      report(pairs(u.state));
    }
  });
}
