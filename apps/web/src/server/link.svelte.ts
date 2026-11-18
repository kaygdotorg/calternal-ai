/**
 * The state of the line to the real server, as the shell shows it: `ok`, `reconnecting` (a request or a live stream could not reach it; it is probed every three seconds until it answers),
 * `expired` (it answered 401: the session ended; the page reloads into the sign-in screen at the same address, which returns there after signing in). Fed by the client's `fetch`
 * (client.ts), so operations, live streams and account calls all report here. Fixtures never touch it.
 */
import { clearCache } from "./cache";
import { ServerError } from "./client";

class Link {
  state = $state<"ok" | "reconnecting" | "expired">("ok");
  /** Start-up asks several things at once, and a signed-out device is answered 401 to some of them: that is a result for boot.ts to read, not a session that ended. */
  quiet = false;
  private hold: Promise<boolean> | null = null;
  /**
   * The app was painted from the device's record before the server confirmed the session (boot.ts): until `confirmed` is true no change is sent (a write, a code, a send), so a session that was revoked elsewhere
   * can never be used for one, and reads, which the server answers only for a live session, go ahead. A refusal ends the wait with `false`.
   */
  holdWrites(confirmed: Promise<boolean>): void { this.hold = confirmed.catch(() => false).then((ok) => { if (ok) this.hold = null; return ok; }); }
  /** Resolves when a change may be sent; rejects (`unauthorized`) when the session was not confirmed. */
  async mayWrite(): Promise<void> { if (this.hold && !(await this.hold)) throw new ServerError("unauthorized", "The session ended."); }
  private timer: ReturnType<typeof setInterval> | null = null;

  /** A request got an answer. */
  ok(): void { if (this.state === "reconnecting") this.settle(); }
  /** A request could not reach the server. */
  failed(): void {
    if (this.state !== "ok") return;
    this.state = "reconnecting";
    this.timer = setInterval(() => void this.probe(), 3000);
  }
  /** The server refused the session. */
  expired(devToken: boolean): void {
    // A development token that is refused cannot be fixed by signing in again; only a cookie session returns to the sign-in screen.
    if (this.state === "expired" || devToken || this.quiet) return;
    this.state = "expired";
    clearCache(); // a stale record must not paint the app again after the reload
    this.settle(false);
    setTimeout(() => location.reload(), 1200);
  }
  private settle(ok = true): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    if (ok) this.state = "ok";
  }
  private async probe(): Promise<void> {
    try {
      const r = await fetch("/api/v1/auth/status", { cache: "no-store" });
      if (!r.ok) return;
      const s = (await r.json()) as { signed_in?: boolean };
      if (s.signed_in === false) this.expired(false); else this.settle();
    } catch { /* still away */ }
  }
}
/** The shared state of the line to the server. */
export const link = new Link();
