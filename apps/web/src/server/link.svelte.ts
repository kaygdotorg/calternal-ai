/**
 * The state of the line to the real server, as the shell shows it: `ok`, `reconnecting` (a request or a live stream could not reach it; it is probed every three seconds until it answers),
 * `expired` (it answered 401: the session ended; the page reloads into the sign-in screen at the same address, which returns there after signing in). Fed by the client's `fetch`
 * (client.ts), so operations, live streams and account calls all report here. Fixtures never touch it.
 */
class Link {
  state = $state<"ok" | "reconnecting" | "expired">("ok");
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
    if (this.state === "expired" || devToken) return;
    this.state = "expired";
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
