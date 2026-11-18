/** What a web push says (docs/web-push.md): the decrypted payload is `{title, status}` and, for a thread's notice, the ids that open it. */
export interface PushPayload {
  title: string;
  status: "needs_you" | "finished" | "test";
  /** The space and the thread the notice is about (UUIDs; `needs_you` and `finished` only): they route the tap and are never shown. A test has neither. */
  space_id?: string;
  thread_id?: string;
}

/** The line shown under the title for a status; `public/push-sw.js` keeps the same words (a test compares them). */
export function notificationLine(status: string): string {
  return status === "needs_you" ? "needs you" : status === "finished" ? "finished" : status === "test" ? "Test notification" : "";
}
