import { openRealServerPage, signUpRealServerOwner, startRealServer, startTlsProxy } from "./lib.mjs";
// Web push against a real server (docs/web-push.md, built with the web-push operations): Settings > Notifications turns notifications on for this device through `push.devices` (the public key) and `push.subscribe`, lists the device, sends a test (`push.test`, a second one within a minute is refused), and turning off removes the device (`push.unsubscribe`). A headless browser has no push service, so the browser's own subscription is a stand-in (a real P-256 key and a provider address the server accepts); everything between the page and the server is real. Env: CAL_BIN, CAL_DIST, CAL_PUSH_PORT.
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_PUSH_PORT ?? 3417);
let bad = 0, server, tls;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const op = (p, name, input) => p.evaluate(async ([name, input]) => { const r = await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: name, input }) }); return r.json(); }, [name, input]);
try {
  // Web push needs a secure public origin: the server is told it is served at https://localhost:<PORT+1> and a TLS front (self-signed) forwards to it.
  server = await startRealServer(PORT, { args: ["--origin", `https://localhost:${PORT + 1}`] });
  tls = await startTlsProxy(server.url, PORT + 1);
  const secure = { ...server, url: tls.url };
  // The service worker script is fetched by the browser itself, which a context option does not cover: the browser ignores certificate errors for this test's self-signed front.
  const b = await chromium.launch({ args: ["--ignore-certificate-errors"] });
  const p = await openRealServerPage(b, { ignoreHTTPSErrors: true });
  // Before anything loads: permission is granted on the tap, and the browser's push subscription is a stand-in with real keys.
  await p.addInitScript(() => {
    let perm = "default"; window.__asked = 0;
    Object.defineProperty(Notification, "permission", { get: () => perm, configurable: true });
    Notification.requestPermission = async () => { window.__asked++; perm = "granted"; return "granted"; };
    let sub = null;
    PushManager.prototype.subscribe = async function () {
      const kp = await crypto.subtle.generateKey({ name: "ECDH", namedCurve: "P-256" }, true, ["deriveBits"]);
      const raw = new Uint8Array(await crypto.subtle.exportKey("raw", kp.publicKey));
      const auth = crypto.getRandomValues(new Uint8Array(16));
      sub = { endpoint: "https://fcm.googleapis.com/fcm/send/e2e-" + crypto.randomUUID(), getKey: (n) => (n === "p256dh" ? raw.buffer : auth.buffer), unsubscribe: async () => { sub = null; return true; } };
      return sub;
    };
    PushManager.prototype.getSubscription = async function () { return sub; };
  });
  await signUpRealServerOwner(p, secure, "Push Owner");
  await p.goto(`${secure.url}/settings/notifications?server=1`); await p.waitForSelector("html[data-ready]");
  await p.getByRole("button", { name: "Turn on for this device" }).waitFor({ timeout: 15000 });
  ok(await p.evaluate(() => window.__asked) === 0, "nothing was asked on load");
  const listed = await op(p, "push.devices", {});
  ok(listed.ok && typeof listed.data?.application_server_key === "string" && listed.data.devices.length === 0, "the server has a public key and no device yet", JSON.stringify(listed).slice(0, 200));
  await p.getByRole("button", { name: "Turn on for this device" }).click();
  await p.getByRole("button", { name: "Send a test" }).waitFor({ timeout: 20000 }).catch(() => {});
  ok(await p.getByRole("button", { name: "Send a test" }).count() === 1, "turning on registers this device (push.subscribe) and the page shows it is on", (await p.getByRole("alert").allInnerTexts().catch(() => [])).join(" | ").slice(0, 160));
  const after = await op(p, "push.devices", {});
  ok(after.data?.devices?.some((d) => d.current), "the server lists this device as the current one", JSON.stringify(after.data?.devices ?? after).slice(0, 200));
  await p.getByRole("button", { name: "Send a test" }).click(); await p.waitForTimeout(800);
  ok(await p.getByText(/test notification is on its way/).count() === 1, "a test is queued (push.test)");
  await p.getByRole("button", { name: "Send a test" }).click(); await p.waitForTimeout(800);
  ok(await p.getByRole("alert").filter({ hasText: /minute/ }).count() === 1, "a second test within a minute is refused in words", (await p.getByRole("alert").allInnerTexts().catch(() => [])).join(" | ").slice(0, 200));
  await p.getByRole("button", { name: "Turn off" }).click(); await p.waitForTimeout(1000);
  ok(await p.getByRole("button", { name: "Turn on for this device" }).count() === 1 && !((await op(p, "push.devices", {})).data?.devices ?? []).length, "turning off removes the device (push.unsubscribe) and puts the page back");
  await b.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
finally { await tls?.stop(); await server?.stop(); }
process.exit(bad ? 1 : 0);
