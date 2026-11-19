/**
 * The interface half of invoke: the modal state the Invoker's hooks drive (a prompt, a picker over the host's labels, the protected confirmation) and the outcome display. One `ActionUi` per open app; `ActionHost.svelte`
 * draws its `dialog`. Outcomes use the design system's toasts: an `ok` with an Undo offers it (the host's, taken through the Invoker), an error, refusal, stale or unknown outcome shows the host's own message.
 */
import { failed, toast, undoable } from "@calternal/ui";
import type { ActionOutcome, ChoiceOptions, ConfirmationMaterial, Invoke } from "../model";
import type { HostSession } from "../wire/session";
import { Invoker, type InvokerHooks } from "./invoker";

/** The dialog now open, or null. One at a time. */
export type Dialog =
  | { kind: "prompt"; name: string; title: string; placeholder?: string; input: "text" | "date" | "datetime"; value: string; done: (v: string | null) => void }
  | { kind: "choose"; name: string; title: string; options: ChoiceOptions["options"]; done: (ref: string | null) => void }
  | { kind: "confirm"; title: string; material: ConfirmationMaterial[]; done: (yes: boolean) => void };

/** The prompts, pickers and confirmation of one app, and the Invoker that uses them. */
export class ActionUi implements InvokerHooks {
  dialog = $state<Dialog | null>(null);
  readonly invoker: Invoker;
  /** The route a `then` opens (the shell sets the route parameter to the created item). */
  onroute?: (name: string, created: string) => void;
  /** An action succeeded: the app re-reads what is drawn. */
  onok?: () => void;

  constructor(session: HostSession) { this.invoker = new Invoker(session, this); }

  /** Counts the dialogs opened; only the newest one closes the dialog slot when answered (a `$state` value is a proxy, so identity cannot be compared). */
  #n = 0;
  /** Opens `d`, first settling the dialog it replaces (cancelled), so no activation is left waiting for ever on a dialog nobody can answer. */
  #open(d: Dialog) { const old = this.dialog; this.dialog = d; if (old) { if (old.kind === "confirm") old.done(false); else old.done(null); } }

  /** Asks for a text, date or datetime answer in the prompt sheet; null when cancelled. */
  prompt(spec: { name: string; title: string; placeholder?: string; kind: "text" | "date" | "datetime"; initial?: string }): Promise<string | null> {
    return new Promise((done) => { const n = ++this.#n; const d: Dialog = { kind: "prompt", name: spec.name, title: spec.title, placeholder: spec.placeholder, input: spec.kind, value: spec.initial ?? "", done: (v) => { if (this.#n === n) this.dialog = null; done(v); } }; this.#open(d); });
  }
  /** Shows the host's option labels and returns the picked ref; null when cancelled. */
  choose(spec: { name: string; title: string; options: ChoiceOptions["options"] }): Promise<string | null> {
    return new Promise((done) => { const n = ++this.#n; const d: Dialog = { kind: "choose", name: spec.name, title: spec.title, options: spec.options, done: (v) => { if (this.#n === n) this.dialog = null; done(v); } }; this.#open(d); });
  }
  /**
   * The protected confirmation. Its title is the host's, fixed: a plugin's action title is plugin text and must not caption a host confirmation, so only the material (which the host wrote) varies. True only on the
   * person's explicit confirmation.
   */
  confirm(spec: { action: Invoke; material: ConfirmationMaterial[] }): Promise<boolean> {
    void spec.action;
    return new Promise((done) => { const n = ++this.#n; const d: Dialog = { kind: "confirm", title: "Please confirm", material: spec.material, done: (v) => { if (this.#n === n) this.dialog = null; done(v); } }; this.#open(d); });
  }
  /** Shows an outcome: Undo for an ok that has one, the host's message for an error, refusal, stale or unknown, and a plain "still working" for a pending outcome that did not settle. */
  outcome(action: Invoke, o: ActionOutcome): void {
    if (o.state === "ok") this.onok?.();
    if (o.state === "ok" && o.undo) { const ref = o.undo.ref; undoable(action.operation, action.id, o.undo.label, async () => { const done = await this.invoker.undo(ref); if (done) this.onok?.(); return done; }); }
    // (an Undo changes the row too: what is drawn is read again, so the next gesture has fresh contexts)
    else if (o.state === "pending") toast("Still working on it. It will finish in the background.");
    else if (o.state === "error" || o.state === "refused" || o.state === "stale" || o.state === "unknown") failed(o.message ?? "That did not happen.");
  }
  route(name: string, created: string): void { this.onroute?.(name, created); }
}
