/**
 * The editor behind a text field (`<input>` or `<textarea>`): stage 1 of docs/editor-port-plan.md. Native editing does the typing (fastest, perfect input methods and accessibility, no bytes);
 * this adds what the block editor will share: every change as a `Transaction` with an origin, one history of the person's own changes (undo and redo work for typed text, a picked
 * mention and a restored draft alike), an input-method gate for changes from elsewhere, and the keystroke probe.
 *
 * Cost per keystroke: one `input` event, one prefix/suffix diff of the text (a pass over the shorter of the old and new text), one history record. No allocation when nothing changed.
 */
import { History, type HistoryOptions } from "./history";
import { ImeGate } from "./ime";
import type { Origin } from "./origins";
import { apply, diff, mapPos, type Change, type Transaction } from "./transaction";
import { startProbe } from "./probe";

export interface FieldOptions extends HistoryOptions {
  /** Called after every change with the change and its origin. */
  onchange?: (tx: Transaction) => void;
}

export class FieldEditor {
  readonly history: History;
  readonly ime = new ImeGate();
  private prev: string;
  private stopProbe: () => void;
  private busy = false;
  constructor(readonly el: HTMLInputElement | HTMLTextAreaElement, private o: FieldOptions = {}) {
    this.history = new History(o);
    this.prev = el.value;
    el.addEventListener("input", this.oninput);
    el.addEventListener("beforeinput", this.onbefore as EventListener);
    el.addEventListener("keydown", this.onkey as EventListener);
    el.addEventListener("compositionstart", this.onstart);
    el.addEventListener("compositionend", this.onend);
    this.stopProbe = startProbe(el);
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
    this.el.removeEventListener("input", this.oninput);
    this.el.removeEventListener("beforeinput", this.onbefore as EventListener);
    this.el.removeEventListener("keydown", this.onkey as EventListener);
    this.el.removeEventListener("compositionstart", this.onstart);
    this.el.removeEventListener("compositionend", this.onend);
    this.stopProbe();
  }

  private step(changes: Change[] | null): boolean {
    if (!changes) return false;
    for (const c of changes) {
      const before = this.el.value;
      this.busy = true;
      this.el.value = apply(before, c);
      this.busy = false;
      const at = c.from + c.insert.length;
      this.el.setSelectionRange(at, at);
      this.prev = this.el.value;
      this.fire(c, "command");
    }
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
  private oninput = () => {
    if (this.busy) return;
    const c = diff(this.prev, this.el.value);
    if (!c) return;
    const before = this.prev;
    this.prev = this.el.value;
    this.history.record(before, c, "user");
    this.o.onchange?.({ ...c, origin: "user" });
  };
  // Undo and redo go through the one history (typed, picked and restored text alike), including the system's own undo gestures.
  private onbefore = (e: InputEvent) => {
    if (e.inputType === "historyUndo") { e.preventDefault(); this.undo(); }
    else if (e.inputType === "historyRedo") { e.preventDefault(); this.redo(); }
  };
  private onkey = (e: KeyboardEvent) => {
    if (e.isComposing || !(e.metaKey || e.ctrlKey) || e.altKey) return;
    const k = e.key.toLowerCase();
    if (k === "z" && !e.shiftKey) { if (this.history.canUndo) { e.preventDefault(); this.undo(); } }
    else if ((k === "z" && e.shiftKey) || k === "y") { if (this.history.canRedo) { e.preventDefault(); this.redo(); } }
  };
  private onstart = () => this.ime.begin();
  private onend = () => { this.ime.end(); this.prev = this.el.value; };
}
