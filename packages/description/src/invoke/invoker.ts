/**
 * Invoke (docs/description-contract.md section 3.2): resolve, confirm when the host challenges, dispatch, and the outcome. The renderer sends the host-issued activation context and ONLY device-held values
 * (form values, prompt answers, a picked choice ref, a selection ref); it never sends a resolved row, route, bind value, target or viewer. Confirmation comes from the host's challenge, never from the description:
 * a catalogue built-in with `atomic_undo` is resolved and dispatched in one exchange, a worker (plugin) operation always takes resolve then dispatch, and a challenge shows the material the host wrote in a protected step
 * (`hooks.confirm`) before `invoke_confirm`. The same activation id is used for every step, so a retry after a dropped connection repeats no effect.
 */
import type { ActionOutcome, ActivationResolution, ChoiceOptions, Interaction, Invoke, ConfirmationMaterial } from "../model";
import type { HostSession } from "../wire/session";
import { oneExchange } from "./catalogue";
import { HostError } from "../wire/transport";

/** What the interface supplies: its own prompts, picker, protected confirmation sheet and outcome display. */
export interface InvokerHooks {
  /** Asks for a text, date or datetime answer; null means the person cancelled. */
  prompt(spec: { name: string; title: string; placeholder?: string; kind: "text" | "date" | "datetime"; initial?: string }): Promise<string | null>;
  /** Shows the labels of the host's options and returns the picked `ref`; null means cancelled. */
  choose(spec: { name: string; title: string; options: ChoiceOptions["options"]; default_ref?: string }): Promise<string | null>;
  /** The protected confirmation: shows exactly the material the host wrote; true only on the person's explicit confirmation. */
  confirm(spec: { action: Invoke; material: ConfirmationMaterial[] }): Promise<boolean>;
  /** The outcome is in (also pending ones when they settle). */
  outcome?(action: Invoke, outcome: ActionOutcome): void;
  /** `then: {route}` after an `ok` that created an item. */
  route?(name: string, created: string): void;
}

/** One page of options the host returned for a choice: the labels to show, the eligible default, and for an extension choice (a harness) the typed ref the placement request needs. */
export interface ChoicePage { options: { ref: string; label: string; harness_ref?: string }[]; default_ref?: string }

/** What the caller holds for one activation. */
export interface Activation {
  /** The host-issued context for this action on the row or view it was drawn for. Absent: nothing is sent. */
  context: string | undefined;
  /** Values on the device: `form` fields of the submitting form, a composer `draft`, a ticked `selection_ref`. */
  interaction?: Pick<Interaction, "form" | "draft" | "selection_ref">;
  /** Called first when a held selection must be sent before the activation (the context then arrives). */
  flush?: () => Promise<void>;
  /** Re-reads the context after `flush`. */
  recontext?: () => string | undefined;
}

/** The local result when nothing was sent (no context yet, cancelled). */
export type Local = { state: "waiting" | "cancelled" };

const newId = (): string => (globalThis.crypto?.randomUUID?.() ?? `act-${Date.now()}-${Math.random().toString(36).slice(2)}`);
const STATUS_POLL_MS = [500, 1000, 2000, 4000];

/** Runs activations: resolve, the protected confirmation when the host asks, dispatch, the outcome. One per open app. */
export class Invoker {
  /** `session` carries the events; `hooks` is the interface's own prompts, picker, confirmation sheet and outcome display. */
  constructor(private session: HostSession, private hooks: InvokerHooks) {}
  /** Activation keys in flight (action id and context): a second activation of the same thing is ignored while one runs. */
  readonly busy = new Set<string>();

  /** Runs one activation to its outcome. Resolves with the outcome, or a local state when nothing was sent. */
  async run(action: Invoke, a: Activation): Promise<ActionOutcome | Local> {
    if (!this.session.current) { // a lapsed lease is refused here, before anything is sent: the host would have to refuse it, and rows that are not current offer no contexts
      const o: ActionOutcome = { activation_id: "", state: "error", code: "lease_lapsed", message: "Reconnecting to the host. Try again in a moment." };
      this.hooks.outcome?.(action, o); return o;
    }
    if (a.flush) { await a.flush(); }
    const context = a.recontext ? a.recontext() : a.context;
    if (!context) return { state: "waiting" }; // no host-issued context for the drawn row: wait for it, send nothing
    const key = `${action.id}|${context}`;
    if (this.busy.has(key)) return { state: "waiting" };
    this.busy.add(key);
    let mayHaveRun = false; // true once a request that could have run the operation has been sent
    try {
      const interaction: Interaction = { ...(a.interaction ?? {}) };
      const activation_id = newId();
      const asked = await this.#ask(action, context, interaction, activation_id);
      if (!asked) return { state: "cancelled" };
      mayHaveRun = oneExchange(action.operation); // a one-exchange resolve is also the dispatch
      let reply = await this.session.exchange<ActionOutcome | ActivationResolution>("activation.resolve", { type: "invoke", action_id: action.id, activation_id, activation_context: context, ...(Object.keys(interaction).length ? { interaction } : {}), ...(oneExchange(action.operation) ? { dispatch: true } : {}) });
      let outcome: ActionOutcome;
      if ("state" in reply) outcome = reply;
      else {
        const res = reply as ActivationResolution;
        let proof: string | undefined;
        if (res.confirmation_challenge) {
          const yes = await this.hooks.confirm({ action, material: res.material ?? [] });
          if (!yes) return { state: "cancelled" };
          const c = await this.session.exchange<{ confirmation_proof: string }>("activation.confirm", { type: "invoke_confirm", activation_id, resolution_ref: res.resolution_ref, confirmation_challenge: res.confirmation_challenge });
          proof = c.confirmation_proof;
        }
        mayHaveRun = true;
        outcome = await this.session.exchange<ActionOutcome>("activation.dispatch", { type: "invoke_dispatch", activation_id, resolution_ref: res.resolution_ref, ...(proof ? { confirmation_proof: proof } : {}) });
      }
      outcome = await this.#settle(outcome);
      this.hooks.outcome?.(action, outcome);
      if (outcome.state === "ok" && outcome.created && action.then?.route) this.hooks.route?.(action.then.route.name, outcome.created);
      return outcome;
    } catch (e) {
      const o = failure(e, mayHaveRun);
      this.hooks.outcome?.(action, o); return o; // an unknown outcome is shown, never retried by the interface
    } finally { this.busy.delete(key); }
  }

  /**
   * Collects prompt answers and choices the inputs name, in the order the description declares them. A `choice` takes its options from a bind (`choice.options`), from the app's extension point
   * (`extension.choices`, for example the installed harnesses) or from the host (`execution_placement`, asked after the harness because placement depends on it: the harness ref travels with the request, with the
   * activation id the person is about to start). Returns false when the person cancelled.
   */
  async #ask(action: Invoke, context: string, into: Interaction, activation_id: string): Promise<boolean> {
    let harness: string | undefined;
    for (const [name, src] of Object.entries(action.inputs ?? {})) {
      if (!("prompt" in src)) continue;
      const p = src.prompt;
      if (p.kind === "choice") {
        const from = (p as { options?: { extension?: string; host?: string } }).options ?? {};
        let page: ChoicePage;
        if (from.extension) page = await this.session.exchange<ChoicePage>("extension.choices", { type: "extension_choices", app: this.session.app ?? "", point: from.extension, activation_context: context, slot: name });
        else if (from.host) page = await this.session.exchange<ChoicePage>("choice.options", { type: "choice_options", activation_context: context, slot: name, activation_id, ...(harness ? { harness } : {}) });
        else page = await this.session.exchange<ChoicePage>("choice.options", { type: "choice_options", activation_context: context, slot: name });
        const ref = await this.hooks.choose({ name, title: p.title, options: page.options, default_ref: page.default_ref });
        if (ref === null) return false;
        if (from.extension) harness = page.options.find((o) => o.ref === ref)?.harness_ref ?? harness;
        (into.choices ??= {})[name] = ref;
      } else {
        const initial = p.initial && "literal" in p.initial && typeof p.initial.literal === "string" ? p.initial.literal : undefined;
        const v = await this.hooks.prompt({ name, title: p.title, placeholder: p.placeholder, kind: (p.kind ?? "text") as "text" | "date" | "datetime", initial });
        if (v === null) return false;
        ((into.prompt ??= {}) as Record<string, unknown>)[name] = v;
      }
    }
    return true;
  }

  /** A pending outcome is polled through the host's own status route until it settles (bounded, backing off); the interface never retries the action. */
  async #settle(o: ActionOutcome): Promise<ActionOutcome> {
    for (let i = 0; o.state === "pending" && o.status_ref && i < 40; i++) {
      await new Promise((r) => setTimeout(r, STATUS_POLL_MS[Math.min(i, STATUS_POLL_MS.length - 1)]));
      try { o = await this.session.exchange<ActionOutcome>("status", { type: "action_status", status_ref: o.status_ref }); } catch { /* keep waiting */ }
    }
    return o;
  }

  /** Takes back an outcome's Undo through the host (`host.undo` is the host's, never a description's). */
  async undo(ref: string): Promise<boolean> {
    try { const o = await this.session.exchange<ActionOutcome>("undo", { type: "undo", undo_ref: ref }); return o.state === "ok"; } catch { return false; }
  }
}

/**
 * What a failed exchange means for the person. The host's own refusal (an HTTP error it wrote: a 4xx, a 409 stale) is shown with the host's words and says nothing ran. A failure before anything that could run the
 * operation was sent (choice options, the resolve) says nothing happened. Only a lost answer AFTER a request that may have run it is "unknown", and the interface then shows that instead of retrying.
 */
export function failure(e: unknown, mayHaveRun: boolean): ActionOutcome {
  if (e instanceof HostError && e.status >= 400 && e.status < 500) return { activation_id: "", state: e.status === 409 ? "stale" : "error", code: e.code ?? `http_${e.status}`, message: e.message };
  if (!mayHaveRun) return { activation_id: "", state: "error", code: "not_started", message: e instanceof HostError ? e.message : "The host did not answer. Nothing was changed." };
  return { activation_id: "", state: "unknown", code: "unreachable", message: "The host did not answer, so it is not known whether this happened." };
}
