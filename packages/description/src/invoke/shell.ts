import type { Action } from "../model";

/** The shell controls a description may ask for through a `ShellAction` (docs/description-contract.md 3.1): set a route parameter, reload, sign out, or open the Trash or share sheet. */
const KINDS = ["route", "reload", "signOut", "sheet"] as const;

/**
 * Whether a non-invoke action is one of the shell controls the contract lets a description request, with exactly one control and a known sheet. Anything else is never passed to the shell. This only says the request is
 * well formed: the shell still decides whether to honour it (a description cannot make the shell sign a person out unasked).
 */
export function shellAllowed(a: Action): boolean {
  if (!("shell" in a) || typeof a.shell !== "object" || a.shell === null) return false;
  const keys = Object.keys(a.shell);
  if (keys.length !== 1 || !(KINDS as readonly string[]).includes(keys[0]!)) return false;
  if ("sheet" in a.shell) return a.shell.sheet === "trash" || a.shell.sheet === "share";
  return true;
}
