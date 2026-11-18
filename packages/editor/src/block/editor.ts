/**
 * The block editor: CodeMirror 6 over plain Markdown text, drawn without visible syntax. It knows nothing about Notes: the host supplies the document, the block kinds (`kinds`), the
 * pickers (`onTrigger`) and what the block menu does (`onMenu`). Changes carry an origin; the person's own typing and commands are undoable, remote and replayed changes never are and wait
 * while an input method composes. Simplified from the existing editor: no Yjs (the state lives on the server), no vim or embeds yet. Frontmatter, block IDs, tables and code grammars are lazy
 * pieces (`features.ts`): call `prepare(text)` before creating an editor or replacing its text so the first draw is already right; the editor also loads one the first time its text needs it.
 */
import { history, redo, undo } from "@codemirror/commands";
import { markdownLanguage } from "@codemirror/lang-markdown";
import { LanguageSupport } from "@codemirror/language";
import { Compartment, EditorState, Prec, type ChangeSpec, type Extension, type Transaction } from "@codemirror/state";
import { EditorView, drawSelection, keymap } from "@codemirror/view";
import type { Origin } from "../origins";
import { ImeGate } from "../ime";
import { probeOn, startProbe } from "../probe";
import { blockAt, blocksIn, deleteBlock, duplicateBlock, moveBlock, type Block, type Kinds } from "./blocks";
import { textAttrs } from "../text/attrs";
import { grouping } from "./grouping";
import { textKeymap } from "./keys";
import { blockChrome } from "./chrome";
import { allLoaded, loadedExtensions, missing, prepare } from "./features";
import { ensureId } from "./ids";
import { headingsOf, type Heading } from "./headings";
import { drawn } from "./hidden";
import { continueList, indentItem } from "./lists";
import { origin } from "./origin";
import { parseAhead } from "./parse";
import { theme } from "./theme";
import { triggers, type Trigger } from "./triggers";

/** What a host gives `createBlockEditor`: the element, the starting text, and callbacks for changes, pickers and block actions. Every callback runs on the main thread inside the editor's update, so keep `onchange` cheap. */
export interface BlockEditorOptions {
  /** The element the editor is built in. */
  parent: HTMLElement;
  /** The starting Markdown. */
  text: string;
  /** Called after every change: the new text, and where it came from. Typing calls it per keystroke; keep it cheap (debounce saving in the host). */
  onchange?: (text: string, origin: Origin) => void;
  /** How to name a kind of block the host knows more about than the parse does (a task is an item that starts with a box). */
  kinds?: Kinds;
  /** The host shows a picker for this trigger (null closes it). */
  onTrigger?: (t: Trigger | null) => void;
  /** While a picker is open the host owns Up, Down, Enter, Tab and Escape: return true to take the key. */
  pickerKey?: (e: KeyboardEvent) => boolean;
  /** The block handle was pressed without dragging. */
  onMenu?: (b: Block, rect: DOMRect) => void;
  /** The accessible name of the text area. */
  label?: string;
  /** True shows the text without letting it change. */
  readOnly?: boolean;
  /** Extra extensions from the host. */
  extensions?: Extension;
}

/** The handle a host holds: read and replace the text, apply changes with an origin, ask about headings and blocks, run block actions as single undoable changes, scroll, focus, undo and redo, and destroy (the host must not call it after that). */
export interface BlockEditor {
  /** The underlying CodeMirror view, for the host's own extensions; do not mutate the document through it (use `apply`). */
  readonly view: EditorView;
  /** The whole current text; reading it builds the string, so avoid doing so per keystroke. */
  readonly value: string;
  /** Replaces the whole text from outside (another note opened); forgets the history. */
  setValue(text: string): void;
  /** Applies a change from code with an origin. A remote or replayed change waits while composing. */
  apply(c: ChangeSpec, o?: Origin): void;
  /** The headings of the document, in order. */
  headings(): Heading[];
  /** The blocks of the document, in order (derived from the parse). */
  blocks(): Block[];
  /** The block that contains a position, or null. */
  blockAt(pos: number): Block | null;
  /** Block actions as single, undoable changes. */
  act(action: "delete" | "duplicate" | "up" | "down", b: Block): void;
  /** Scrolls a position into view without moving the caret. */
  scrollTo(pos: number): void;
  /** Gives the block whose last line holds `pos` a block ID if it has none (what the first link to a block does, ADR 0056 B1) as one undoable change, and returns the ID. */
  ensureBlockId(pos: number): string;
  /** Puts the caret in the editor. */
  focus(): void;
  /** Undoes the last step; returns false when there is nothing to undo. */
  undo(): boolean;
  /** Redoes the last undone step; returns false when there is nothing to redo. */
  redo(): boolean;
  /** Removes the editor and its listeners; the host must stop calling it afterwards. */
  destroy(): void;
}

/** Builds the block editor in `o.parent`. Synchronous and cheap for a note of any length (the view renders only the viewport); the lazy features a note needs are loaded beforehand by `prepare` so the first draw is already right. */
export function createBlockEditor(o: BlockEditorOptions): BlockEditor {
  const ime = new ImeGate();
  const own = (tr: Transaction): Origin => tr.annotation(origin) ?? "user";
  const lazy = new Compartment(); // the lazy pieces loaded so far (features.ts)
  const extensions = (): Extension[] => [
    new LanguageSupport(markdownLanguage),
    lazy.of(loadedExtensions()),
    history({ newGroupDelay: 600 }),
    grouping,
    drawSelection(),
    EditorView.lineWrapping,
    EditorView.contentAttributes.of({ "aria-label": o.label ?? "Note", "aria-multiline": "true", ...textAttrs("prose") }),
    EditorState.readOnly.of(!!o.readOnly),
    EditorView.editable.of(!o.readOnly),
    parseAhead,
    drawn,
    theme,
    blockChrome({ kinds: o.kinds, onMenu: o.onMenu }),
    ...(o.onTrigger ? [triggers(o.onTrigger)] : []),
    // The host's picker keys first, then lists, then the defaults.
    Prec.highest(EditorView.domEventHandlers({ keydown: (e) => (o.pickerKey ? o.pickerKey(e) : false) })),
    Prec.high(keymap.of([{ key: "Enter", run: continueList }, { key: "Tab", run: indentItem(1) }, { key: "Shift-Tab", run: indentItem(-1) }])),
    ...textKeymap,
    EditorView.updateListener.of((u) => {
      if (!u.docChanged) return;
      const tr = u.transactions.find((t) => t.docChanged)!;
      o.onchange?.(u.state.doc.toString(), own(tr));
      // A piece the new text needs (the first table typed, a fence with a language, a task) loads now. Only a change that could have made one needed is looked at, so plain typing pays one
      // comparison: everything already loaded, or no character in the change that a piece is made of, ends it here.
      if (allLoaded()) return;
      let hint = false;
      u.changes.iterChanges((_a, _b, fb, _tb, ins) => { if (hint) return; const t = ins.toString(); if (/[|`~^\]\n-]/.test(t) || (t && /^[ \t]*(```|~~~)/.test(u.state.doc.lineAt(fb).text))) hint = true; });
      if (!hint) return;
      let from = u.state.doc.length, to = 0;
      u.changes.iterChangedRanges((_a, _b, fb, tb) => { from = Math.min(from, fb); to = Math.max(to, tb); });
      const d = u.state.doc;
      const first = Math.max(1, d.lineAt(Math.min(from, d.length)).number - 1), last = Math.min(d.lines, d.lineAt(Math.min(to, d.length)).number + 1);
      const near = d.sliceString(d.line(first).from, Math.min(d.line(last).to, d.line(first).from + 4000));
      if (missing(near).length) void prepare(near).then((n) => { if (n) refresh(); });
    }),
    o.extensions ?? [],
  ];
  const view = new EditorView({ state: EditorState.create({ doc: o.text, extensions: extensions() }), parent: o.parent });
  const refresh = () => { if (view.dom.isConnected) view.dispatch({ effects: lazy.reconfigure(loadedExtensions()) }); };
  const stopProbe = startProbe(view.contentDOM);
  view.contentDOM.addEventListener("compositionstart", () => ime.begin());
  view.contentDOM.addEventListener("compositionend", () => ime.end());

  const apply = (c: ChangeSpec, org: Origin = "command") => {
    const run = () => view.dispatch({ changes: c, annotations: [origin.of(org)], ...(org === "remote" || org === "replay" ? { addToHistory: false } : {}) });
    if ((org === "remote" || org === "replay") && (ime.active || view.composing)) ime.hold(run); else run();
  };
  const api: BlockEditor = {
    view,
    get value() { return view.state.doc.toString(); },
    setValue(text) { view.setState(EditorState.create({ doc: text, extensions: extensions() })); if (missing(text).length) void prepare(text).then((n) => { if (n && view.state.doc.toString() === text) refresh(); }); },
    apply,
    headings: () => headingsOf(view.state),
    blocks: () => blocksIn(view.state, 0, view.state.doc.length, o.kinds),
    blockAt: (pos) => blockAt(view.state, pos, o.kinds),
    act(action, b) {
      const s = view.state;
      const bs = blocksIn(s, 0, s.doc.length, o.kinds);
      const i = bs.findIndex((x) => x.from === b.from);
      if (action === "delete") apply(deleteBlock(s, b));
      else if (action === "duplicate") apply(duplicateBlock(s, b));
      else if (action === "up" && i > 0) { const t = bs[i - 1]!; apply(moveTo(s, b, t.from)); }
      else if (action === "down" && i >= 0 && i < bs.length - 1) { const t = bs[i + 1]!; apply(moveTo(s, b, Math.min(s.doc.length, t.to + 1))); }
    },
    ensureBlockId(pos) { const r = ensureId(view.state, pos); if (r.change) apply(r.change); return r.id; },
    scrollTo(pos) { view.dispatch({ effects: EditorView.scrollIntoView(pos, { y: "start", yMargin: 8 }) }); },
    focus() { view.focus(); },
    undo: () => undo(view),
    redo: () => redo(view),
    destroy() { stopProbe(); view.destroy(); },
  };
  if (probeOn()) (globalThis as unknown as { __block?: BlockEditor }).__block = api; // for the e2e checks (?test=1)
  return api;
}

function moveTo(s: EditorState, b: Block, target: number): ChangeSpec { return moveBlock(s, b, target) ?? []; }
