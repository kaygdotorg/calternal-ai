// The real-server adapter against a real `calternal serve --development`: the app reads the server's spaces and notes through /api (proxied; the Host and Origin checks pass),
// makes and edits a note, and follows the live stream when another client writes. Development token mode (sign-in is not wired for browsers yet).
// Usage: CAL_SERVER=http://127.0.0.1:3277 CAL_TOKEN_FILE=<data-dir>/dev-tokens/owner.token node e2e/server-notes.mjs   (after `npm run build`; never prints the token).
import { readFileSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const token = readFileSync(process.env.CAL_TOKEN_FILE, "utf8").trim();
const SERVER = process.env.CAL_SERVER;
const stop = await serve();
const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const ctx = await b.newContext({ ...DEVICES.desktop });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
const page = await ctx.newPage();
await page.goto(`${BASE}/notes?server=1&token=${encodeURIComponent(token)}`);
await page.waitForSelector("html[data-ready]");
await page.waitForSelector('nav[aria-label="Notes"] .sp');
ok("the sidebar says it is the server's, with the space's name", /server/.test(await page.locator('nav[aria-label="Notes"] .sp').innerText()), await page.locator('nav[aria-label="Notes"] .sp').innerText());
ok("the token left the address", !page.url().includes("token="));
const before = await page.locator('nav[aria-label="Notes"] a.row').count();
await page.getByRole("button", { name: "New note" }).click();
await page.waitForSelector(".cm-h1");
await page.waitForTimeout(500);
ok("a new note is created on the server and opened", (await page.locator(".cm-h1").first().innerText()) === "Untitled" && (await page.locator('nav[aria-label="Notes"] a.row').count()) === before + 1);
const typeDoc = async (text) => { await page.locator(".cm-content").click(); await page.keyboard.press("Control+a"); await page.keyboard.type(text); };
await typeDoc("# Written in the browser\n\nA paragraph saved with a base revision.");
await page.waitForTimeout(2300);
ok("an edit is autosaved (notes.update with the base revision)", (await page.locator(".cm-h1").first().innerText()) === "Written in the browser" && (await page.getByText("A paragraph saved with a base revision.").count()) === 1);
// Another client writes; the live stream makes this page refresh its list.
const space = await page.evaluate(async (t) => { const r = await fetch("/api/v1/operations", { method: "POST", headers: { authorization: `Bearer ${t}`, "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: "spaces.list", input: {} }) }); return (await r.json()).data.spaces[0].id; }, token);
await page.evaluate(async ([t, sp]) => { await fetch("/api/v1/operations", { method: "POST", headers: { authorization: `Bearer ${t}`, "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: "notes.create", input: { space_id: sp, title: "From another client", markdown: "hello" } }) }); }, [token, space]);
await page.waitForFunction(() => [...document.querySelectorAll('nav[aria-label="Notes"] a.row')].some((a) => a.textContent.includes("From another client")), null, { timeout: 8000 }).then(() => ok("a note written elsewhere appears through the live stream", true), () => ok("a note written elsewhere appears through the live stream", false));
// A draft survives a reload (drafts.save / drafts.get), and a write made while the server cannot be reached is queued (pending.queue) and sent later (pending.submit).
await typeDoc("# Written in the browser\n\nAn unsent line.");
await page.waitForTimeout(900);
await page.reload(); await page.waitForSelector(".cm-content");
await page.waitForSelector(".draft");
ok("an unsent draft is offered back after a reload (a private draft on the server)", (await page.locator(".draft").innerText()).includes("unsent draft"));
// Cut the notes.update call only (the queue call must get through), as if the connection dropped at that moment.
await page.route("**/api/v1/operations", async (route) => { const op = JSON.parse(route.request().postData() ?? "{}").operation; if (op === "notes.update") await route.abort("connectionrefused"); else await route.continue(); });
await page.getByRole("button", { name: "Restore" }).click();
ok("restoring brings the text back", (await page.locator(".cm-content").innerText()).includes("An unsent line."));
await page.waitForSelector(".wait", { timeout: 9000 });
ok("an update that could not be sent waits (pending.queue)", (await page.locator(".wait .wt").first().innerText()).includes("Written in the browser"));
await page.unroute("**/api/v1/operations");
await page.getByRole("button", { name: "Send now" }).click();
await page.waitForFunction(() => !document.querySelector(".wait"), null, { timeout: 8000 }).then(() => ok("Send now submits it and the queue empties (pending.submit)", true), () => ok("Send now submits it and the queue empties (pending.submit)", false));
ok("the note shows what was queued", (await page.locator(".cm-content").innerText()).includes("An unsent line."));
// describe and a wrong token
const d = await page.evaluate(async (t) => { const r = await fetch("/api/v1/describe", { headers: { authorization: `Bearer ${t}` } }); return (await r.json()).operations?.length; }, token);
ok("describe lists the operations", d > 20, `${d} operations`);
const bad401 = await page.evaluate(async () => (await fetch("/api/v1/describe", { headers: { authorization: "Bearer wrong" } })).status);
ok("a wrong token is refused", bad401 === 401 || bad401 === 403, `status ${bad401}`);
await b.close(); stop(); process.exit(bad ? 1 : 0);
