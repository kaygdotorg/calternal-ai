// The onboarding cannot be skipped by a reload, and nothing stored on the address by another installation (or another app) can decide what this one shows. The server is mocked at the HTTP edge (auth/status, auth/me, describe,
// a few operations), so this runs on the built app alone.
//  (a) a reload at each onboarding step resumes that step, never the app;
//  (b) stale state from another installation: with no owner yet everything stored (storage, IndexedDB, caches, service workers) is wiped and the onboarding starts at its first step; a record of another account is dropped;
//  (c) the paint-now path: a device that "knows the account" (a fresh `cal.boot` record) still never paints the app while an onboarding record is open.
// Usage: node e2e/with-preview.mjs e2e/onboarding-resume.mjs
import { BASE, launch, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const json = (route, body, status = 200) => route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
const ME = (id, extra = {}) => ({ user: { id, name: "Kay", username: "kay", role: "owner", capabilities: ["spaces.read"] }, installation: "inst-1", ...extra });
async function mock(page, { setup, signedIn, account = "acc-1", me = {} }) {
  await page.route("**/api/v1/auth/status", (r) => json(r, { setup, signed_in: signedIn }));
  await page.route("**/api/v1/describe", (r) => json(r, { protocol_version: 1, genui_version: 1, principal: {}, operations: [{ id: "onboarding.state" }] }));
  await page.route("**/api/v1/auth/me", (r) => signedIn ? json(r, ME(account, me)) : json(r, { error: { code: "unauthorized", message: "no session" } }, 401));
  await page.route("**/api/v1/operations", async (r) => {
    const op = JSON.parse(r.request().postData() ?? "{}").operation;
    if (op === "spaces.list") return json(r, { ok: true, data: { spaces: [{ id: "sp-1", name: "Personal", personal: true, role: "owner" }] } });
    if (op === "onboarding.state") return json(r, { ok: true, data: { finished: !!globalThis.__finished } });
    if (op === "machines.onboarding") return json(r, { ok: true, data: { attached: true, ready: false, machine_id: null } });
    return json(r, { ok: false, error: { code: "not_found", message: "none" } });
  });
}
const heading = (p) => p.locator("main h1, main h2").first().innerText().catch(() => "");
async function fresh(init) {
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 } });
  const page = await ctx.newPage();
  if (init) await page.addInitScript(init.fn, init.arg);
  return { ctx, page };
}
const onboardingUp = async (p) => { await p.waitForSelector("html[data-ready]"); await p.locator("main ol.steps").waitFor({ timeout: 8000 }).catch(() => {}); return (await p.locator("main ol.steps").count()) > 0; };
try {
  // (a) reload at each step
  for (const step of ["look", "machine", "provider"]) {
    const { ctx, page } = await fresh({ fn: (rec) => { if (!sessionStorage.getItem("seeded")) { sessionStorage.setItem("seeded", "1"); localStorage.setItem("cal.onboarding", JSON.stringify(rec)); } }, arg: { who: "owner", coder: false, step, done: ["you", "look", "machine"].slice(0, ["look", "machine", "provider"].indexOf(step) + 1 - (step === "look" ? 1 : 0)), skipped: [], account: "acc-1" } });
    await mock(page, { setup: false, signedIn: true });
    await page.goto(BASE + "/?server=1");
    ok(await onboardingUp(page), `(a) the onboarding shows at the "${step}" step, not the app`);
    for (let i = 0; i < 2; i++) { await page.reload(); await onboardingUp(page); }
    const cur = await page.locator("ol.steps li[aria-current=step]").innerText().catch(() => "");
    ok(await onboardingUp(page) && new RegExp(step === "look" ? "Look" : step === "machine" ? "Machine" : "Provider", "i").test(cur), `(a) two reloads later it is still the "${step}" step`, cur.replace(/\s+/g, " "));
    ok((await page.locator(".shell").count()) === 0, `(a) and the app never mounted`);
    await ctx.close();
  }
  // (a2) passkey made, page reloaded before the step moved on: who-you-are is done, the next step shows
  { const { ctx, page } = await fresh({ fn: () => { if (!sessionStorage.getItem("seeded")) { sessionStorage.setItem("seeded", "1"); localStorage.setItem("cal.onboarding", JSON.stringify({ who: "owner", coder: false, step: "you", done: [], skipped: [] })); } } });
    await mock(page, { setup: false, signedIn: true });
    await page.goto(BASE + "/?server=1");
    ok(await onboardingUp(page) && /Recovery/i.test(await page.locator("ol.steps li[aria-current=step]").innerText()), "(a) a reload right after the passkey goes on to the next step, the recovery codes (the account exists)");
    await ctx.close(); }
  // (b) stale state, no owner yet: seeded on a plain file of the origin (no app running), then the app opens
  { const { ctx, page } = await fresh(null);
    await ctx.route("**/old-sw.js", (r) => r.fulfill({ contentType: "text/javascript", body: "self.addEventListener('fetch',()=>{});" }));
    await mock(page, { setup: true, signedIn: false });
    await page.goto(BASE + "/boot.js");
    await page.evaluate(async () => {
      localStorage.setItem("cal.onboarding", JSON.stringify({ who: "owner", coder: false, step: "provider", done: ["you", "look", "machine"], skipped: [], account: "old-acc" }));
      localStorage.setItem("cal.boot", JSON.stringify({ account: "old-acc", capabilities: [], spaces: [{ id: "x", name: "Old" }], at: Date.now() }));
      localStorage.setItem("cal.sidebar", "stale");
      await new Promise((r) => { const q = indexedDB.open("old-app-db", 1); q.onsuccess = q.onerror = () => r(); });
      await navigator.serviceWorker.register("/old-sw.js", { scope: "/" }); await navigator.serviceWorker.ready; await caches.open("old-app-shell");
    });
    const before = await page.evaluate(async () => ({ sws: (await navigator.serviceWorker.getRegistrations()).length, caches: (await caches.keys()).length }));
    await page.goto(BASE + "/?server=1");
    await onboardingUp(page); await page.waitForTimeout(1500);
    const after = await page.evaluate(async () => ({ onboarding: localStorage.getItem("cal.onboarding"), boot: localStorage.getItem("cal.boot"), sidebar: localStorage.getItem("cal.sidebar"), sws: (await navigator.serviceWorker.getRegistrations()).length, caches: (await caches.keys()).length, dbs: (await indexedDB.databases()).map((d) => d.name) }));
    ok(before.sws >= 1 && before.caches >= 1, "(b) the test origin really holds a stale service worker and cache", JSON.stringify(before));
    ok(after.boot === null && after.sidebar === null && !/old-acc/.test(after.onboarding ?? ""), "(b) with no owner yet, the old account record, the old onboarding record and the old keys are gone", JSON.stringify({ boot: after.boot, sidebar: after.sidebar }));
    ok(after.sws === 0 && after.caches === 0 && !after.dbs.includes("old-app-db"), "(b) and no service worker, cache or database of another installation remains", JSON.stringify({ sws: after.sws, caches: after.caches, dbs: after.dbs }));
    ok(await onboardingUp(page) && /You/i.test(await page.locator("ol.steps li[aria-current=step]").innerText()), "(b) the onboarding starts at its first step");
    await ctx.close(); }
  // (b2) signed in, but the record belongs to another account
  { const { ctx, page } = await fresh({ fn: () => { if (!sessionStorage.getItem("seeded")) { sessionStorage.setItem("seeded", "1"); localStorage.setItem("cal.onboarding", JSON.stringify({ who: "owner", coder: false, step: "provider", done: ["you", "look", "machine"], skipped: [], account: "old-acc" })); } } });
    await mock(page, { setup: false, signedIn: true });
    await page.goto(BASE + "/?server=1"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
    const rec = await page.evaluate(() => localStorage.getItem("cal.onboarding"));
    ok(!/old-acc/.test(rec ?? "") && !/Provider/i.test(await page.locator("ol.steps li[aria-current=step]").innerText().catch(() => "")), "(b) a record of another account is not resumed", String(rec).slice(0, 60));
    await ctx.close(); }
  // (c) paint-now path
  { const { ctx, page } = await fresh({ fn: () => { if (!sessionStorage.getItem("seeded")) { sessionStorage.setItem("seeded", "1"); localStorage.setItem("cal.boot", JSON.stringify({ account: "acc-1", capabilities: ["spaces.read"], spaces: [{ id: "sp-1", name: "Personal", role: "owner" }], at: Date.now() })); localStorage.setItem("cal.onboarding", JSON.stringify({ who: "owner", coder: false, step: "machine", done: ["you", "look"], skipped: [], account: "acc-1" })); } } });
    await mock(page, { setup: false, signedIn: true });
    const mounted = []; await page.exposeFunction("sawShell", () => mounted.push(1));
    await page.addInitScript(() => { new MutationObserver(() => { if (document.querySelector(".shell")) window.sawShell(); }).observe(document, { childList: true, subtree: true }); });
    await page.goto(BASE + "/?server=1");
    ok(await onboardingUp(page), "(c) a device that knows the account still shows the onboarding");
    ok(mounted.length === 0, "(c) the app never painted, not even for a moment");
    await ctx.close(); }
  // (e) the server remembers: an account it says has not finished onboarding resumes it on any browser (no local record), and one that has finished goes to the app
  for (const finished of [false, true]) {
    globalThis.__finished = finished;
    const { ctx, page } = await fresh({ fn: () => localStorage.setItem("cal.firstrun", "1") });
    await mock(page, { setup: false, signedIn: true });
    await page.goto(BASE + "/?server=1"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1200);
    const onb = (await page.locator("main ol.steps").count()) > 0;
    ok(finished ? !onb && (await page.locator(".shell").count()) > 0 : onb && /Recovery/i.test(await page.locator("ol.steps li[aria-current=step]").innerText()), `(e) the server says finished=${finished}: ${finished ? "the app" : "the onboarding resumes at the recovery codes"}`);
    await ctx.close();
  }
  globalThis.__finished = undefined;
  // (d) the server's policy needs a verified email and this signed-in account has none: ask before the app (the login was not refused), and on a server with no mail say so plainly
  for (const mail of [true, false]) {
    const { ctx, page } = await fresh({ fn: () => localStorage.setItem("cal.firstrun", "1") });
    await mock(page, { setup: false, signedIn: true, me: { email_setup_required: true, mail_available: mail, email_status: "missing" } });
    await page.goto(BASE + "/?server=1"); await page.waitForSelector("html[data-ready]");
    await page.getByRole("heading", { name: "Add your email address" }).waitFor({ timeout: 8000 }).catch(() => {});
    const t = await page.locator("main").innerText().catch(() => "");
    ok(/Add your email address/.test(t) && (await page.locator(".shell").count()) === 0, `(d) email_setup_required asks for an address before the app (mail ${mail ? "available" : "not configured"})`);
    ok(mail ? !/cannot send email/.test(t) : /cannot send email, so an address cannot be verified here/.test(t), `(d) the no-mail copy shows only when mail_available is false`);
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
