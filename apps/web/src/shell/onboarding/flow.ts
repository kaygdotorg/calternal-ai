/**
 * The first-owner onboarding as data: which steps there are, in what order for whom, what each may do when skipped, and where the person has got to, kept on this device so a reload resumes (docs/ui-shell.md
 * "Onboarding"). Pure: no screens here. The style is infraternal's setup wizard: numbered steps that turn into ticks, a step confirms and moves on by itself, waits show a count-up, nothing is faked.
 */
/** One step. */
export type StepId = "you" | "look" | "machine" | "provider" | "done";
/** Who is onboarding: the first owner of a fresh installation, or a person who followed an invitation. */
export type Who = "owner" | "invited";

/** What a step is called in the stepper and as its heading's name. */
export const NAMES: Record<StepId, string> = { you: "You", look: "Look", machine: "Machine", provider: "Provider", done: "Done" };

/** The steps in order: the owner's five (who you are and your passkey, a look, a machine, a provider's sign-in, done) and the invited person's three (you, a look, done). */
export function path(who: Who): StepId[] { return who === "owner" ? ["you", "look", "machine", "provider", "done"] : ["you", "look", "done"]; }

/**
 * Whether a step may be skipped. Who you are cannot (it makes the account); the look is a preference (skipping keeps the default); a machine and a provider can be added later in Settings (skipping says what is
 * lost: no thread can run until there is one). Done is the end.
 */
export const skippable: Record<StepId, boolean> = { you: false, look: true, machine: true, provider: true, done: false };

/** What a skipped step costs, in words, shown beside Skip. */
export const SKIP_NOTE: Partial<Record<StepId, string>> = {
  look: "You can change the look any time in Settings, Appearance.",
  machine: "Without a machine no thread can run. You can add one later in Settings, Machines.",
  provider: "Without a provider a machine cannot answer. You can sign in later in Settings, Machines.",
};

/** The step after `from` on this path, or null at the end. */
export function after(who: Who, from: StepId): StepId | null { const p = path(who); const i = p.indexOf(from); return i >= 0 && i < p.length - 1 ? p[i + 1]! : null; }
/** The step before `from`, or null at the first. */
export function before(who: Who, from: StepId): StepId | null { const p = path(who); const i = p.indexOf(from); return i > 0 ? p[i - 1]! : null; }

/** Where the person is: the path, the current step, and which steps are done or skipped. */
export interface Saved { who: Who; step: StepId; done: StepId[]; skipped: StepId[] }
const KEY = "cal.onboarding";

/** The saved state of this device, or null when there is none (never started, or finished). */
export function load(storage: Pick<Storage, "getItem"> = localStorage): Saved | null {
  try {
    const s = JSON.parse(storage.getItem(KEY) ?? "null") as Saved | null;
    if (!s || (s.who !== "owner" && s.who !== "invited") || !path(s.who).includes(s.step)) return null;
    return { who: s.who, step: s.step, done: s.done?.filter((x) => path(s.who).includes(x)) ?? [], skipped: s.skipped?.filter((x) => path(s.who).includes(x)) ?? [] };
  } catch { return null; }
}
/** Keeps the state on this device (private mode: for this page only). */
export function save(s: Saved, storage: Pick<Storage, "setItem"> = localStorage): void { try { storage.setItem(KEY, JSON.stringify(s)); } catch { /* not kept */ } }
/** Forgets it: the onboarding is finished. */
export function clear(storage: Pick<Storage, "removeItem"> = localStorage): void { try { storage.removeItem(KEY); } catch { /* none */ } }
