/**
 * The interface half of invoke: the modal state the Invoker's hooks drive (a prompt, a picker over the host's labels, the protected confirmation) and the outcome display. One `ActionUi` per open app; `ActionHost.svelte`
 * draws its `dialog`. Outcomes use the design system's toasts: an `ok` with an Undo offers it (the host's, taken through the Invoker), an error, refusal, stale or unknown outcome shows the host's own message.
 */
import { failed, undoable } from "@calternal/ui";
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

  prompt(spec: { name: string; title: string; placeholder?: string; kind: "text" | "date" | "datetime"; initial?: string }): Promise<string | null> {
    return new Promise((done) => { this.dialog = { kind: "prompt", name: spec.name, title: spec.title, placeholder: spec.placeholder, input: spec.kind, value: spec.initial ?? "", done: (v) => { this.dialog = null; done(v); } }; });
  }
  choose(spec: { name: string; title: string; options: ChoiceOptions["options"] }): Promise<string | null> {
    return new Promise((done) => { this.dialog = { kind: "choose", name: spec.name, title: spec.title, options: spec.options, done: (v) => { this.dialog = null; done(v); } }; });
  }
  confirm(spec: { action: Invoke; material: ConfirmationMaterial[] }): Promise<boolean> {
    return new Promise((done) => { this.dialog = { kind: "confirm", title: spec.action.title ?? "Confirm", material: spec.material, done: (v) => { this.dialog = null; done(v); } }; });
  }
  outcome(action: Invoke, o: ActionOutcome): void {
    if (o.state === "ok") this.onok?.();
    if (o.state === "ok" && o.undo) { const ref = o.undo.ref; undoable(action.operation, action.id, o.undo.label, async () => { const done = await this.invoker.undo(ref); if (done) this.onok?.(); return done; }); }
    // (an Undo changes the row too: what is drawn is read again, so the next gesture has fresh contexts)
    else if (o.state === "error" || o.state === "refused" || o.state === "stale" || o.state === "unknown") failed(o.message ?? "That did not happen.");
  }
  route(name: string, created: string): void { this.onroute?.(name, created); }
}
