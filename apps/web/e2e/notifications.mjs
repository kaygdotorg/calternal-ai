// Settings > Notifications (fixtures): nothing is asked on load; "Turn on for this device" asks the browser for permission once and lists this device; Send a test; the other device can be removed; turning off removes this one. Where push cannot work (an iPhone tab) the page says to install the app. The service worker file is push-only.
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (const device of ["desktop", "phone"]) {
    const { page: p, ctx } = await open(browser, device, "dark", "/settings/notifications");
    await p.addInitScript(() => { window.__asked = 0; Object.defineProperty(Notification, "permission", { get: () => window.__perm ?? "default", configurable: true }); Notification.requestPermission = async () => { window.__asked++; window.__perm = "granted"; return "granted"; }; });
    await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(800);
    ok(await p.getByRole("button", { name: "Turn on for this device" }).count() === 1, `${device}: the page offers to turn notifications on for this device`);
    ok(await p.evaluate(() => window.__asked) === 0, `${device}: nothing was asked on load`);
    ok(await p.getByText("Kay's laptop (Firefox)").count() === 1, `${device}: another device is listed`);
    await p.getByRole("button", { name: "Turn on for this device" }).click(); await p.waitForTimeout(500);
    ok(await p.evaluate(() => window.__asked) === 1, `${device}: the permission was asked on the tap, once`);
    ok(await p.getByRole("status").filter({ hasText: "on for this device" }).count() === 1 && await p.getByRole("button", { name: "Send a test" }).count() === 1, `${device}: it is on, and a test can be sent`);
    await p.getByRole("button", { name: "Send a test" }).click(); await p.waitForTimeout(300);
    ok(await p.getByText(/test notification is on its way/).count() === 1, `${device}: a test is said to be on its way`);
    await p.getByRole("button", { name: "Send a test" }).click(); await p.waitForTimeout(300);
    ok(await p.getByRole("alert").filter({ hasText: /minute/ }).count() === 1, `${device}: a second test within a minute is refused in plain words`);
    await p.getByRole("button", { name: "Remove" }).click(); await p.waitForTimeout(300);
    ok(await p.getByText("Kay's laptop (Firefox)").count() === 0, `${device}: removing the other device takes it off the list`);
    await p.getByRole("button", { name: "Turn off" }).click(); await p.waitForTimeout(300);
    ok(await p.getByRole("button", { name: "Turn on for this device" }).count() === 1, `${device}: turning off puts the page back`);
    await ctx.close();
  }
  { const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148 Safari/604.1" });
    await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    const p = await ctx.newPage(); await p.goto(process.env.CAL_URL + "/settings/notifications"); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(800);
    ok(await p.locator("[data-reason=ios-not-installed]").count() === 1 && /Add to Home Screen/.test(await p.locator("[data-reason=ios-not-installed]").innerText()), "an iPhone tab is told to install the app (Add to Home Screen) and offered no switch");
    ok(await p.getByRole("button", { name: "Turn on for this device" }).count() === 0, "and the turn-on button is not there");
    await ctx.close(); }
  { const r = await fetch(process.env.CAL_URL + "/push-sw.js"); const t = await r.text();
    ok(r.ok && !/addEventListener\(\s*["']fetch["']/.test(t), "the push worker is served and has no fetch handler"); }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
finally { await browser.close(); await stop(); }
process.exit(bad ? 1 : 0);
