/**
 * Undo and redo of the person's own changes only (ADR 0022): foreign changes (remote, replayed) are never undone by this person's undo, and positions are mapped through them.
 * Typing coalesces into one step while the keys keep coming (a pause of `idleMs`, a different kind of change, or a command ends a step); a command's change is always one step.
 * Bounded: at most `limit` steps. Pure: it records changes and returns the change to apply; it never touches the DOM.
 */
import { invert, mapPos, type Change } from "./transaction";
import type { Origin } from "./origins";

interface Step { changes: Change[]; at: number; kind: "typing" | "command" }
export interface HistoryOptions { idleMs?: number; limit?: number; now?: () => number }

export class History {
  private undoStack: Step[] = [];
  private redoStack: Step[] = [];
  private idle: number;
  private limit: number;
  private now: () => number;
  constructor(o: HistoryOptions = {}) { this.idle = o.idleMs ?? 600; this.limit = o.limit ?? 200; this.now = o.now ?? (() => performance.now()); }

  /** Records that `c` was applied to `before` (the text before it) with `origin`. A foreign change only shifts what is already recorded. */
  record(before: string, c: Change, origin: Origin): void {
    if (origin === "remote" || origin === "replay") {
      for (const s of this.undoStack) s.changes = s.changes.map((x) => ({ ...x, from: mapPos(x.from, c), to: mapPos(x.to, c) }));
      for (const s of this.redoStack) s.changes = s.changes.map((x) => ({ ...x, from: mapPos(x.from, c), to: mapPos(x.to, c) }));
      return;
    }
    const kind = origin === "command" ? "command" : "typing";
    const inv = invert(before, c);
    const top = this.undoStack[this.undoStack.length - 1];
    const t = this.now();
    this.redoStack = [];
    if (top && kind === "typing" && top.kind === "typing" && t - top.at <= this.idle && contiguous(top.changes[0]!, c)) {
      top.changes.unshift(inv);
      top.at = t;
    } else {
      this.undoStack.push({ changes: [inv], at: t, kind });
      if (this.undoStack.length > this.limit) this.undoStack.shift();
    }
  }
  get canUndo(): boolean { return this.undoStack.length > 0; }
  get canRedo(): boolean { return this.redoStack.length > 0; }

  /** The changes (in the order to apply them) that undo the last step; `text` is the current text, used to record the redo. Null when there is nothing to undo. */
  undo(text: string): Change[] | null { return this.move(this.undoStack, this.redoStack, text); }
  /** The changes that redo the last undone step. */
  redo(text: string): Change[] | null { return this.move(this.redoStack, this.undoStack, text); }
  /** Forgets everything (the text was replaced from outside). */
  clear(): void { this.undoStack = []; this.redoStack = []; }

  private move(from: Step[], to: Step[], text: string): Change[] | null {
    const step = from.pop();
    if (!step) return null;
    const applied: Change[] = [];
    const inverse: Change[] = [];
    let cur = text;
    for (const c of step.changes) {
      inverse.unshift(invert(cur, c));
      cur = cur.slice(0, c.from) + c.insert + cur.slice(c.to);
      applied.push(c);
    }
    to.push({ changes: inverse, at: this.now(), kind: "command" });
    return applied;
  }
}

/** A typing change continues the step when it lands at or next to where the previous one ended (the previous inverse's end). */
function contiguous(prevInverse: Change, c: Change): boolean {
  return c.from === prevInverse.to || c.to === prevInverse.from || (c.from <= prevInverse.to && c.to >= prevInverse.from);
}
