// This app's only service worker: it receives a web push and shows it, nothing else. It has no fetch handler and no cache, so it can never answer a navigation with an old page (the old
// app's worker did, and public/sw.js removes it); the page registers it from Settings > Notifications, on the person's tap. The payload is `{title, status}` and nothing more by design
// (docs/web-push.md): no message text. A `needs_you` or `finished` payload also carries the space's and the thread's UUIDs as `space_id` and `thread_id`: they route the tap and are never shown. A tap focuses an open window
// and takes it to that thread, or opens one; a test (no ids) or an older server opens the Inbox.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

/** The line under the title for a status: "needs you", "finished", a test. Kept the same as src/notify/payload.ts (a test compares them). */
function lineFor(status) {
  return status === "needs_you" ? "needs you" : status === "finished" ? "finished" : status === "test" ? "Test notification" : "";
}

self.addEventListener("push", (event) => {
  let data = {};
  try { data = event.data ? event.data.json() : {}; } catch { /* not JSON: shown as a bare notice */ }
  const title = typeof data.title === "string" && data.title ? data.title : "Calternal";
  // Where a tap goes: the thread when the payload names one (a UUID, checked before it becomes part of an address), else the Inbox. Kept in the notification's data, not in its text.
  const thread = typeof data.thread_id === "string" && /^[0-9a-f-]{32,40}$/i.test(data.thread_id) ? data.thread_id : "";
  const space = typeof data.space_id === "string" && /^[0-9a-f-]{32,40}$/i.test(data.space_id) ? data.space_id : "";
  event.waitUntil(self.registration.showNotification(title, { body: lineFor(data.status), tag: "calternal-" + (thread || String(data.status || "notice")), icon: "/icons/icon-192.png", badge: "/icons/icon-192.png", data: { thread, space } }));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const thread = event.notification.data && event.notification.data.thread;
  const path = thread ? "/threads/" + thread : "/threads";
  event.waitUntil((async () => {
    const all = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    for (const c of all) { if ("focus" in c) { await c.focus(); if ("navigate" in c) await c.navigate(path).catch(() => {}); return; } }
    await self.clients.openWindow(path);
  })());
});
