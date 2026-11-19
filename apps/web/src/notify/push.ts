/**
 * Browser push for this device (docs/web-push.md): whether it can work here, turning it on (the permission is asked on that tap and never on load), the person's devices and removing one, and a test. The browser
 * side (permission, service worker, subscription) is shared; the account side is a client with two implementations: the fixtures' in-memory list and the server's `push.*` operations.
 */
import { server, serverMode } from "../server/client";

/** One of the person's devices that receives notifications. */
export interface PushDevice { id: string; name: string; current: boolean; expiresAt: number }

/** Why push cannot work here, or null when it can. */
export type Unsupported = { reason: "ios-not-installed" | "no-push" | "insecure"; text: string } | null;

/** Whether this browser can do push, and what to tell the person when it cannot. iPhone and iPad only get push in the app installed to the Home Screen. */
export function supportOf(env: { ua: string; standalone: boolean; secure: boolean; hasPush: boolean; hasNotification: boolean }): Unsupported {
  const ios = /iPhone|iPad|iPod/.test(env.ua) || (/Macintosh/.test(env.ua) && /Mobile/.test(env.ua));
  if (!env.secure) return { reason: "insecure", text: "Notifications need a secure (https) address." };
  if (ios && !env.standalone) return { reason: "ios-not-installed", text: "On iPhone and iPad, notifications only work in the installed app: tap Share, choose Add to Home Screen, then open Calternal from the Home Screen and come back here." };
  if (!env.hasPush || !env.hasNotification) return { reason: "no-push", text: "This browser cannot receive notifications." };
  return null;
}

/** The environment as this page sees it. */
export function currentEnv() {
  return {
    ua: navigator.userAgent,
    standalone: (navigator as unknown as { standalone?: boolean }).standalone === true || matchMedia("(display-mode: standalone)").matches,
    secure: isSecureContext,
    hasPush: "serviceWorker" in navigator && "PushManager" in window,
    hasNotification: "Notification" in window,
  };
}

/** The account's side: the devices, adding this one, removing one, a test. */
export interface PushClient {
  /** The person's devices and the server's public key (null on fixtures). */
  devices(): Promise<{ key: string | null; devices: PushDevice[] }>;
  /** Registers this browser's subscription for this installation. Returns the new list. */
  subscribe(sub: { endpoint: string; p256dh: string; auth: string }): Promise<PushDevice[]>;
  /** Removes a device. False when it was not there. */
  remove(id: string): Promise<boolean>;
  /** Asks for a test notification to this installation. */
  test(): Promise<void>;
}

interface WireDevice { device_id: string; name: string; current: boolean; expires_at_ms: number }
const deviceOf = (d: WireDevice): PushDevice => ({ id: d.device_id, name: d.name, current: d.current, expiresAt: d.expires_at_ms });

/** The server's `push.*` operations. */
export const serverPush: PushClient = {
  async devices() { const r = await server.operate<{ application_server_key: string; devices: WireDevice[] }>("push.devices", {}); return { key: r.application_server_key, devices: r.devices.map(deviceOf) }; },
  async subscribe(sub) { const r = await server.operate<{ devices: WireDevice[] }>("push.subscribe", sub); return r.devices.map(deviceOf); },
  async remove(id) { return (await server.operate<{ removed: boolean }>("push.unsubscribe", { device_id: id })).removed; },
  async test() { await server.operate("push.test", {}); },
};

/** The fixtures' account: a list in memory (one other device already there), so the page can be seen and tested without a server. A test is only said to be queued. */
export function fixturePush(): PushClient {
  let devices: PushDevice[] = [{ id: "d-laptop", name: "Kay's laptop (Firefox)", current: false, expiresAt: Date.now() + 80 * 86_400_000 }];
  let lastTest = 0;
  return {
    async devices() { return { key: null, devices: [...devices] }; },
    async subscribe() { devices = [...devices.filter((d) => !d.current), { id: "d-here", name: "This device", current: true, expiresAt: Date.now() + 90 * 86_400_000 }]; return [...devices]; },
    async remove(id) { const had = devices.some((d) => d.id === id); devices = devices.filter((d) => d.id !== id); return had; },
    async test() { if (Date.now() - lastTest < 60_000) throw new Error("A test was sent a moment ago; try again in a minute."); lastTest = Date.now(); },
  };
}

/** The client for this page: the server's in server mode, the fixtures' otherwise. */
let fixture: PushClient | null = null;
export const pushClient = (): PushClient => (serverMode() ? serverPush : (fixture ??= fixturePush()));

const b64 = (bytes: ArrayBuffer | null): string => bytes ? btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "") : "";
const fromB64 = (s: string): Uint8Array => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(s.length / 4) * 4, "=")), (c) => c.charCodeAt(0));

/** This browser's push registration, if it has one. Registers nothing. */
export async function currentSubscription(): Promise<PushSubscription | null> {
  try { const reg = await navigator.serviceWorker.getRegistration("/push-sw.js") ?? (await navigator.serviceWorker.getRegistrations()).find((r) => r.active?.scriptURL.endsWith("/push-sw.js")); return (await reg?.pushManager.getSubscription()) ?? null; } catch { return null; }
}

/** Turns notifications on for this device. Call it from the tap that asks for it: the permission prompt is shown here and nowhere else. Returns the person's devices after, or throws a plain sentence. */
export async function turnOn(client: PushClient): Promise<PushDevice[]> {
  if (Notification.permission === "denied") throw new Error("Notifications are blocked for this site. Allow them in your browser's site settings, then try again.");
  const permission = Notification.permission === "granted" ? "granted" : await Notification.requestPermission();
  if (permission !== "granted") throw new Error("Notifications were not allowed, so nothing was turned on.");
  const { key } = await client.devices();
  if (!serverMode() || !key) return client.subscribe({ endpoint: "fixture://this-device", p256dh: "", auth: "" });
  const reg = await navigator.serviceWorker.register("/push-sw.js", { scope: "/" });
  await navigator.serviceWorker.ready;
  const sub = await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: fromB64(key) as BufferSource });
  return client.subscribe({ endpoint: sub.endpoint, p256dh: b64(sub.getKey("p256dh")), auth: b64(sub.getKey("auth")) });
}

/** Turns notifications off for this device: the browser's subscription is dropped and the device is removed from the account. */
export async function turnOff(client: PushClient, id: string): Promise<boolean> {
  try { const s = await currentSubscription(); await s?.unsubscribe(); } catch { /* the browser had none */ }
  return client.remove(id);
}
