/**
 * Failures in plain words with the next step, in one place (the CLI rule for errors: say what happened, then what to do). A server `ServerError` is told by its stable `code`, never by its message; a
 * sign-in one by the same field (`ServerError` carries both). `doing` is the thing that was being attempted, in the lowercase ("load the members"), for the cases that need it.
 */
import { ServerError } from "./client";

/** The first owner's message: there is no owner yet, so nobody can send a link; the code comes from the server itself (a code lasts 24 hours; asking again replaces it). */
export const FIRST_OWNER = "That setup code is wrong or has expired. On the server, as the deploy account, run: podman exec calternal-ai /usr/local/bin/calternal setup-code --appdata /data/appdata  and paste the code it prints (it lasts 24 hours; running it again replaces it).";

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
  passkey_unavailable: "Passkeys are not available in this app or browser yet. Open Calternal in Safari or Chrome and continue there.",
  passkey_origin: "This page's address cannot be used for passkeys. Open Calternal at the address your server gave you (https, always the same name), then try again.",
  passkey_unsupported: "This device or browser does not support passkeys. Use Safari, Chrome or Edge on a recent phone or computer.",
  passkey_failed: "That passkey was not recognised. Use the passkey you made for this server, or ask for a new setup link if this is a new installation.",
  setup_token_invalid: FIRST_OWNER,
  invite_invalid: "That invitation is wrong, expired or already used. Ask whoever invited you for a new link and paste its code.",
  too_many_requests: "Too many tries. Wait a minute and try again.",
  rate_limited: "Too many tries. Wait a minute and try again.",
  last_passkey: "Keep at least one passkey, or you could not sign in again. Add another first.",
  passkey_reverification_required: "Confirm with your passkey, then try again.",
};

/** What to tell the person about a failed `doing`. While setup is pending (`firstOwner`) any answer that says the code is not valid, whichever of the server's two codes it is (`setup_token_invalid`, or `invite_invalid` for a well-formed code it does not know), gets the first owner's message: nobody can send a link yet. */
export function say(e: unknown, doing = "do that", opts: { firstOwner?: boolean } = {}): string {
  const code = e instanceof ServerError ? e.code : e instanceof TypeError ? "unavailable" : "";
  if (opts.firstOwner && (code === "invite_invalid" || code === "setup_token_invalid")) return FIRST_OWNER;
  return BY_CODE[code] ?? `Could not ${doing}. Try again.`;
}
