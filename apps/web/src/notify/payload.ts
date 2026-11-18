/** What a web push says (docs/web-push.md): the decrypted payload is `{title, status}` and nothing else. */
export interface PushPayload { title: string; status: "needs_you" | "finished" | "test" }

/** The line shown under the title for a status; `public/push-sw.js` keeps the same words (a test compares them). */
export function notificationLine(status: string): string {
  return status === "needs_you" ? "needs you" : status === "finished" ? "finished" : status === "test" ? "Test notification" : "";
}
