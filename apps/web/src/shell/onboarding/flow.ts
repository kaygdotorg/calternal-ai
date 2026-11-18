/**
 * The first-owner onboarding as data: which steps there are, in what order for whom, what each may do when skipped, and where the person has got to, kept on this device so a reload resumes (docs/ui-shell.md
 * "Onboarding"). Pure: no screens here. The style is infraternal's setup wizard: numbered steps that turn into ticks, a step confirms and moves on by itself, waits show a count-up, nothing is faked.
 */
/** One step. */
export type StepId = "you" | "recovery" | "passkey2" | "look" | "machine" | "provider" | "repos" | "cli" | "direct" | "done";
/** Who is onboarding: the first owner of a fresh installation, or a person who followed an invitation. */
export type Who = "owner" | "invited";

/** What a step is called in the stepper and as its heading's name. */
export const NAMES: Record<StepId, string> = { you: "You", recovery: "Recovery", passkey2: "Second passkey", look: "Look", machine: "Machine", provider: "Provider", repos: "Repositories", cli: "CLI", direct: "Direct", done: "Done" };

/** The steps in order: one path for everyone (who you are and your passkey, a look, a machine, a provider's sign-in, done), and for a person who writes code (the switch "I write code") three more before done: repositories, the CLI, and direct mode on their own machine. */
export function path(coder: boolean): StepId[] { return coder ? ["you", "recovery", "passkey2", "look", "machine", "provider", "repos", "cli", "direct", "done"] : ["you", "recovery", "passkey2", "look", "machine", "provider", "done"]; }

/**
 * Whether a step may be skipped. Who you are cannot (it makes the account); the look is a preference (skipping keeps the default); a machine and a provider can be added later in Settings (skipping says what is
 * lost: no thread can run until there is one); the coding steps are all optional. Done is the end.
 */
export const skippable: Record<StepId, boolean> = { you: false, recovery: false, passkey2: true, look: true, machine: true, provider: true, repos: true, cli: true, direct: true, done: false };

/** What a skipped step costs, in words, shown beside Skip. */
export const SKIP_NOTE: Partial<Record<StepId, string>> = {
  passkey2: "With one passkey, losing that device means using a recovery code. You can add another any time in Settings, Account.",
  look: "You can change the look any time in Settings, Appearance.",
  machine: "Without a machine no thread can run. You can add one later in Settings, Machines.",
  provider: "Without a provider a machine cannot answer. You can sign in later in Settings, Machines.",
  repos: "You can add repositories later, in the first thread.",
  cli: "The CLI can be installed any time; Settings, Machines shows the command.",
  direct: "Threads stay sandboxed. You can change it later in Settings, Machines.",
};

/** The step after `from` on this path, or null at the end. */
export function after(coder: boolean, from: StepId): StepId | null { const p = path(coder); const i = p.indexOf(from); return i >= 0 && i < p.length - 1 ? p[i + 1]! : null; }
/** The step before `from`, or null at the first. */
export function before(coder: boolean, from: StepId): StepId | null { const p = path(coder); const i = p.indexOf(from); return i > 0 ? p[i - 1]! : null; }

/** Where the person is: who they are (an owner or an invited person: it only changes the words), whether they write code (the longer path), the current step, and which steps are done or skipped. */
export interface Saved { who: Who; coder: boolean; step: StepId; done: StepId[]; skipped: StepId[]; /** The account this record belongs to (set once the account exists): a record of another account, from another installation at the same address, is never resumed. */ account?: string }
const KEY = "cal.onboarding";

/** The saved state of this device, or null when there is none (never started, or finished). */
export function load(storage: Pick<Storage, "getItem"> = localStorage): Saved | null {
  try {
    const s = JSON.parse(storage.getItem(KEY) ?? "null") as Saved | null;
    if (!s || (s.who !== "owner" && s.who !== "invited")) return null;
    const coder = !!s.coder, p = path(coder);
    if (!p.includes(s.step)) return null;
    return { who: s.who, coder, step: s.step, done: s.done?.filter((x) => p.includes(x)) ?? [], skipped: s.skipped?.filter((x) => p.includes(x)) ?? [], ...(typeof s.account === "string" ? { account: s.account } : {}) };
  } catch { return null; }
}
/** Keeps the state on this device (private mode: for this page only). */
export function save(s: Saved, storage: Pick<Storage, "setItem"> = localStorage): void { try { storage.setItem(KEY, JSON.stringify(s)); } catch { /* not kept */ } }
/** Forgets it: the onboarding is finished. */
export function clear(storage: Pick<Storage, "removeItem"> = localStorage): void { try { storage.removeItem(KEY); } catch { /* none */ } }

/**
 * Matches a stored record to the signed-in account. A record of another account is another installation's (or another person's) and is dropped. A record with no account is only believable at the first step, written
 * before the account existed: the passkey was made and the page reloaded before the step moved on, so who-you-are is done and the recovery codes (shown once, never kept) are the next thing. A record with no account at any later step cannot be this person's and is dropped.
 */
export function reconcile(s: Saved | null, account: string): Saved | null {
  if (!s) return null;
  if (s.account) return s.account === account ? s : null;
  if (s.step !== "you") return null;
  return { ...s, account, step: "recovery", done: s.done.includes("you") ? s.done : [...s.done, "you"] };
}
