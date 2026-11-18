// This app's only service worker: it receives a web push and shows it, nothing else. It has no fetch handler and no cache, so it can never answer a navigation with an old page (the old
// app's worker did, and public/sw.js removes it); the page registers it from Settings > Notifications, on the person's tap. The payload is `{title, status}` and nothing more by design
// (docs/web-push.md): no message text, no thread id. A tap opens the app at the Inbox, where the thread that needs you or has finished is listed.
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
  event.waitUntil(self.registration.showNotification(title, { body: lineFor(data.status), tag: "calternal-" + String(data.status || "notice"), icon: "/icons/icon-192.png", badge: "/icons/icon-192.png" }));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil((async () => {
    const all = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    for (const c of all) { if ("focus" in c) { await c.focus(); if ("navigate" in c) await c.navigate("/threads").catch(() => {}); return; } }
    await self.clients.openWindow("/threads");
  })());
});
