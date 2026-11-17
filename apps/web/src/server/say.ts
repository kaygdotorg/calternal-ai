/**
 * Failures in plain words with the next step, in one place (the CLI rule for errors: say what happened, then what to do). A server `ServerError` is told by its stable `code`, never by its message; a
 * sign-in one by the same field (`ServerError` carries both). `doing` is the thing that was being attempted, in the lowercase ("load the members"), for the cases that need it.
 */
import { ServerError } from "./client";

const BY_CODE: Record<string, string> = {
  unauthorized: "Your session ended. Sign in again.",
  forbidden: "You are not allowed to do that here. Ask the space's owner or a manager to do it, or to give you a role that can.",
  not_found: "That is gone, or you cannot see it. Go back and try again.",
  conflict: "Someone changed it at the same time. Reload and try again.",
  invalid_input: "That did not look right. Check what you entered and try again.",
  too_large: "That is too large. Make it smaller and try again.",
  unavailable: "Can't reach the server. Check the connection; this page keeps trying.",
  internal: "The server had a problem. Try again in a moment; if it keeps happening, tell whoever runs it.",
  // Sign-in (auth routes use their own codes).
  cancelled: "The passkey prompt was closed or timed out. Try again, and choose a passkey when your device asks.",
  passkey_failed: "That passkey was not recognised. Use the passkey you made for this server, or ask for a new setup link if this is a new installation.",
  setup_token_invalid: "That setup code is wrong or already used. Ask for a new setup link and paste its code.",
  invite_invalid: "That setup code is wrong or already used. Ask for a new setup link and paste its code.",
  too_many_requests: "Too many tries. Wait a minute and try again.",
  last_passkey: "Keep at least one passkey, or you could not sign in again. Add another first.",
  passkey_reverification_required: "Confirm with your passkey, then try again.",
};

/** What to tell the person about a failed `doing`. */
export function say(e: unknown, doing = "do that"): string {
  const code = e instanceof ServerError ? e.code : e instanceof TypeError ? "unavailable" : "";
  return BY_CODE[code] ?? `Could not ${doing}. Try again.`;
}
