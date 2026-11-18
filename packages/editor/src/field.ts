/**
 * The editor behind a text field (`<input>` or `<textarea>`): stage 1 of docs/editor-port-plan.md. Native editing does the typing (fastest, perfect input methods and accessibility, no bytes);
 * this adds what the block editor will share: every change as a `Transaction` with an origin, one history of the person's own changes (undo and redo work for typed text, a picked
 * mention and a restored draft alike), an input-method gate for changes from elsewhere, and the keystroke probe.
 *
 * Cost per keystroke: one `input` event, one prefix/suffix diff of the text (a pass over the shorter of the old and new text), one history record. No allocation when nothing changed.
 */
import { History, type HistoryOptions } from "./history";
import { COMMANDS } from "./text/commands";
import { stringDoc, type Edit } from "./text/doc";
import { isolatedInput } from "./text/grouping";
import { matchKey } from "./text/keys";
import { applyTextAttrs, type TextKind } from "./text/attrs";
import { matchTrigger, type TriggerKind } from "./text/triggers";
import { ImeGate } from "./ime";
import type { Origin } from "./origins";
import { apply, diff, mapPos, type Change, type Transaction } from "./transaction";
import { probeOn, startProbe } from "./probe";

/** A picker trigger at the caret (the range covers the trigger characters and what follows them). */
export interface FieldTrigger { kind: TriggerKind; query: string; from: number; to: number }

/** How a plain field behaves: change callback, what it holds (for the browser's spelling rules), whether Escape collapses a selection, picker triggers, and the history's limits. */
export interface FieldOptions extends HistoryOptions {
  /** Called after every change with the change and its origin. */
  onchange?: (tx: Transaction) => void;
  /** What the field holds, for the browser's spelling and capitalisation (default: prose). */
  kind?: TextKind;
  /** Escape collapses a selection to the caret (as it does in the block editor). Off when Escape means "cancel" to the host. */
  escapeCollapses?: boolean;
  /** Picker triggers to watch for at the caret; `on` is called with the trigger, or null when there is none, whenever that changes. */
  triggers?: { kinds: readonly TriggerKind[]; on: (t: FieldTrigger | null) => void };
}

/** The note editor's text behaviour (undo and redo as steps, IME-safe, picker triggers) for an `<input>` or `<textarea>`, so every field in the app edits the same way. Attaches listeners to the element; `destroy` removes them. */
export class FieldEditor {
  readonly history: History;
  readonly ime = new ImeGate();
  private prev: string;
  private stopProbe: () => void;
  private busy = false;
  constructor(readonly el: HTMLInputElement | HTMLTextAreaElement, private o: FieldOptions = {}) {
    this.history = new History(o);
    this.prev = el.value;
    // Capture, so this runs before a framework's own `input` listener (Svelte's `bind:value`): its follow-up (`sync`) must see the change as already recorded, or it takes the typing for a replacement from outside and forgets the history.
    el.addEventListener("input", this.oninput, true);
    el.addEventListener("beforeinput", this.onbefore as EventListener);
    el.addEventListener("keydown", this.onkey as EventListener);
    el.addEventListener("compositionstart", this.onstart);
    el.addEventListener("compositionend", this.onend);
    el.addEventListener("paste", this.onpaste as EventListener);
    if (o.triggers) for (const t of ["input", "keyup", "pointerup", "focus"]) el.addEventListener(t, this.ontrigger);
    applyTextAttrs(el, o.kind ?? "prose");
    this.stopProbe = startProbe(el);
    if (probeOn()) ((globalThis as unknown as { __fields?: FieldEditor[] }).__fields ??= []).push(this); // for the e2e checks (?test=1)
  }
  get value(): string { return this.el.value; }

  /** Applies a change from code (a picked mention, a remote edit, a replayed one). A foreign change waits while the person is composing. Resolves with false when it had to wait. */
  apply(c: Change, origin: Origin = "command"): boolean {
    const run = () => {
      const before = this.el.value;
      const caret = [this.el.selectionStart ?? before.length, this.el.selectionEnd ?? before.length] as const;
      this.busy = true;
      this.el.value = apply(before, c);
      this.busy = false;
      // The caret follows the change the way text does: before it stays, after it shifts; the person's own insert leaves it after the inserted text.
      const own = origin === "command" || origin === "user";
      const at = own ? c.from + c.insert.length : mapPos(caret[0], c);
      this.el.setSelectionRange(own ? at : mapPos(caret[0], c), own ? at : mapPos(caret[1], c));
      this.record(before, c, origin);
    };
    if ((origin === "remote" || origin === "replay") && this.ime.active) { this.ime.hold(run); return false; }
    run();
    return true;
  }
  /** Replaces the whole text from outside (a draft restored, a send cleared the field) and forgets the history, like a native field does. */
  reset(text: string): void {
    if (this.el.value !== text) { this.busy = true; this.el.value = text; this.busy = false; }
    this.prev = text;
    this.history.clear();
  }
  /**
   * Called after the text may have been replaced from outside (a framework binding set it): when it differs from what this editor last saw and no input event explains it,
   * the history is forgotten, like a native field. A no-op after the person's own typing.
   */
  sync(): void {
    if (this.el.value !== this.prev) { this.prev = this.el.value; this.history.clear(); }
  }
  undo(): boolean { return this.step(this.history.undo(this.el.value)); }
  redo(): boolean { return this.step(this.history.redo(this.el.value)); }
  destroy(): void {
    this.el.removeEventListener("input", this.oninput, true);
    this.el.removeEventListener("beforeinput", this.onbefore as EventListener);
    this.el.removeEventListener("keydown", this.onkey as EventListener);
    this.el.removeEventListener("compositionstart", this.onstart);
    this.el.removeEventListener("compositionend", this.onend);
    this.stopProbe();
  }

  private step(changes: Change[] | null): boolean {
    if (!changes) return false;
    let lo = Infinity, hi = -Infinity;
    for (const c of changes) {
      const before = this.el.value;
      this.busy = true;
      this.el.value = apply(before, c);
      this.busy = false;
      lo = Math.min(lo, c.from);
      hi = Math.max(hi, c.from + c.insert.length);
      this.prev = this.el.value;
      this.fire(c, "command");
    }
    // Like the block editor: after an undo or redo the text it brought back is selected (everything the step touched), and where it only removed text the caret sits where it was.
    this.el.setSelectionRange(lo, hi);
    return true;
  }
  private record(before: string, c: Change, origin: Origin) {
    this.prev = this.el.value;
    this.history.record(before, c, origin);
    this.fire(c, origin);
  }
  private fire(c: Change, origin: Origin) {
    this.el.dispatchEvent(new Event("input", { bubbles: true })); // lets bindings (`bind:value`) see programmatic changes
    this.o.onchange?.({ ...c, origin });
  }
  private lastInput = "";
  private dragging = false;
  private oninput = () => {
    if (this.busy) return;
    const c = diff(this.prev, this.el.value);
    if (!c) return;
    const before = this.prev;
    this.prev = this.el.value;
    // Grouping is the shared rule (text/grouping.ts): a paste, a drop or a replacement is a step of its own; a drop that moves text is one step.
    const own = isolatedInput(this.lastInput) || this.lastInput === "deleteByDrag";
    const join = this.lastInput === "insertFromDrop" && this.dragging;
    if (this.lastInput === "deleteByDrag") this.dragging = true; else if (this.lastInput === "insertFromDrop") this.dragging = false;
    this.history.record(before, c, own ? "command" : "user", { join });
    this.o.onchange?.({ ...c, origin: own ? "command" : "user" });
  };
  // Undo and redo go through the one history (typed, picked and restored text alike), including the system's own undo gestures.
  private onbefore = (e: InputEvent) => {
    this.lastInput = e.inputType;
    if (e.inputType === "historyUndo") { e.preventDefault(); this.undo(); }
    else if (e.inputType === "historyRedo") { e.preventDefault(); this.redo(); }
  };
  private onkey = (e: KeyboardEvent) => {
    if (this.ime.composingKey(e)) return;
    // The shared shortcuts (text/keys.ts): line commands where there are lines, and Escape collapsing a selection where Escape does not mean cancel.
    const id = matchKey(e, this.el instanceof HTMLTextAreaElement);
    if (id && (id !== "collapse" || this.o.escapeCollapses)) {
      const doc = stringDoc(this.el.value);
      const a = this.el.selectionStart ?? 0, z = this.el.selectionEnd ?? 0, back = this.el.selectionDirection === "backward";
      const edit = COMMANDS[id](doc, back ? { anchor: z, head: a } : { anchor: a, head: z });
      if (edit) { e.preventDefault(); e.stopPropagation(); this.edit(edit); }
      return;
    }
    if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
    const k = e.key.toLowerCase();
    if (k === "z" && !e.shiftKey) { if (this.history.canUndo) { e.preventDefault(); this.undo(); } }
    else if ((k === "z" && e.shiftKey) || k === "y") { if (this.history.canRedo) { e.preventDefault(); this.redo(); } }
  };
  /** Applies a command's edit: its change as one undo step (none for a pure selection change) and then its selection. */
  edit(e: Edit): void {
    const c = e.change;
    if (c.insert !== "" || c.to !== c.from) this.apply(c, "command");
    const { anchor, head } = e.selection;
    this.el.setSelectionRange(Math.min(anchor, head), Math.max(anchor, head), head < anchor ? "backward" : "forward");
  }
  /** A paste into a one-line field turns line breaks into spaces (a one-line field cannot hold them); into a many-line field it is native. Either way it is a step of its own. */
  private onpaste = (e: ClipboardEvent) => {
    if (this.el instanceof HTMLTextAreaElement) return;
    const t = e.clipboardData?.getData("text/plain") ?? "";
    if (!/[\r\n]/.test(t)) return;
    e.preventDefault();
    this.apply({ from: this.el.selectionStart ?? 0, to: this.el.selectionEnd ?? 0, insert: t.replace(/\s*[\r\n]+\s*/g, " ").trim() }, "command");
  };
  private lastTrigger = "";
  private ontrigger = () => {
    const t = this.o.triggers;
    if (!t) return;
    const caret = this.el.selectionStart ?? 0;
    const sel = this.el.selectionEnd ?? caret;
    const v = this.el.value;
    const ls = v.lastIndexOf("\n", caret - 1) + 1;
    const m = sel === caret ? matchTrigger(v.slice(ls, caret), t.kinds) : null;
    const key = m ? `${m.kind}:${caret}:${m.query}` : "";
    if (key === this.lastTrigger) return;
    this.lastTrigger = key;
    t.on(m ? { kind: m.kind, query: m.query, from: caret - m.query.length - m.lead, to: caret } : null);
  };
  /** True while a key event belongs to input-method composition (including the key that confirms it): a host must not submit, cancel or move on a key for which this is true. */
  composing(e: KeyboardEvent): boolean { return this.ime.composingKey(e); }
  private onstart = () => this.ime.begin();
  private onend = () => { this.ime.end(); this.prev = this.el.value; };
}
