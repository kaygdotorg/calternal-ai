/**
 * Who is signed in, for the account picture and its menu: the name, the username and the address the person gave, read once from the server (`account.me`). Fixtures answer with a pretend person so the screens
 * can be looked at. Until the answer comes the picture shows an initial of the pretend name rather than nothing; a failed read leaves it as it was (the account menu then still offers Sign out).
 */
import { devToken, serverMode } from "../server/client";

class Self {
  /** The person's name as they gave it. */
  name = $state("Kay");
  /** Their username (the handle others find them by). */
  username = $state("kay");
  /** Their email address, when they gave one. */
  email = $state<string | null>(null);
  private asked = false;
  /** Reads the signed-in account once; safe to call from every place that shows it. */
  load(): void {
    if (this.asked || !serverMode() || devToken()) return;
    this.asked = true;
    void import("../server/auth").then((a) => a.account.me()).then((m) => { this.name = m.user.name || m.user.username; this.username = m.user.username ?? this.username; this.email = m.email ?? null; }, () => { this.asked = false; });
  }
}
/** The signed-in person (see the file's comment). */
export const self = new Self();

/**
 * Signs out of this browser after asking. Against a cookie session it ends the session on the server and reloads into the sign-in screen; with a development token or on fixtures it only says so (nothing is changed).
 * Used by the account menu and by Settings, Account: one place, one wording.
 */
export function signOutNow(): void {
  if (!serverMode() || devToken()) { alert("Sign out\n\nThis is a fixture or a development session: nothing is changed. The real action signs this browser out."); return; }
  if (!confirm("Sign out of Calternal on this device? You will need your passkey to get back in.")) return;
  void import("../server/auth").then((a) => a.signOut()).then(() => location.replace(import.meta.env.BASE_URL), () => alert("Could not sign out. Check the connection and try again."));
}
