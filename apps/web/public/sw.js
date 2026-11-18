// A kill switch for a service worker this address may still hold. The existing Calternal app registered `/sw.js`, which precaches its own pages and answers every navigation with them; this app registers no service
// worker, so the file at that address is replaced by this one: the browser's own update check fetches it, it takes over at once, deletes every cache, unregisters itself and makes open pages load from the network.
// (The app also removes any service worker and cache at start-up, src/server/origin-state.ts; this file is for the visit where the old worker answers before the app can run.)
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) await caches.delete(key);
    await self.registration.unregister();
    for (const client of await self.clients.matchAll({ type: "window" })) client.navigate(client.url);
  })());
});
