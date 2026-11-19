// The server goes away for a few seconds in the middle of a save (an update: the proxy answers 502; or the connection drops), against a small stand-in for the server's wire protocol (operations, the two event
// streams, the account routes; development-token mode). The page shows the reconnecting state, the save waits and is repeated with its ORIGINAL request id, and the note ends as Saved (never Not saved), with the
// change made exactly once on the server. Also: the server's own refusal is shown at once, not waited out. Usage: node e2e/server-gap.mjs   (after `npm run build`; it starts its own preview with the proxy pointed at the stand-in)
import http from "node:http";
import { createServer } from "node:net";
import { chromium } from "playwright";
const free = () => new Promise((res) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); });
const PORT = await free(), SPACE = "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b11", NOTE = "019a8c00-0000-7000-8000-000000000001";
const now = () => new Date().toISOString();
const note = { id: NOTE, space_id: SPACE, title: "Journal replay notes", markdown: "# Journal replay notes\n\nThe journal is read twice.\n", revision: "r1", updated_at: now() };
let away = null;           // null, "502" or "drop": how the stand-in is away right now
const seen = [];            // the request id of every notes.update that reached the server while it was up
const attempts = [];        // ... and of every one that was sent at all, while it was away or not: [request_id, away mode]
const applied = new Map();  // request_id -> result (the server's receipts)
const unhandled = new Set();
const sse = new Set();
const send = (res, status, body, type = "application/json") => { res.writeHead(status, { "content-type": type }); res.end(typeof body === "string" ? body : JSON.stringify(body)); };
const server = http.createServer((req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${PORT}`);
  if (url.pathname === "/__away") { away = url.searchParams.get("mode") || null; return send(res, 200, { away }); }
  if (url.pathname === "/__state") return send(res, 200, { seen, applied: [...applied.keys()], attempts, unhandled: [...unhandled], markdown: note.markdown });
  if (away) {
    // Away: nothing is answered properly, but what was sent is remembered, so the test can see the repeats.
    if (req.method === "POST" && url.pathname === "/api/v1/operations") { let b = ""; req.on("data", (d) => (b += d)); req.on("end", () => { try { const q = JSON.parse(b); if (q.operation === "notes.update") attempts.push([q.request_id, away]); } catch {} if (away === "drop") req.socket.destroy(); else send(res, 502, "<html><body>502 Bad Gateway</body></html>", "text/html"); }); return; }
    if (away === "drop") return req.socket.destroy();
    return send(res, 502, "<html><body>502 Bad Gateway</body></html>", "text/html");
  }
  if (url.pathname === "/api/v1/describe") return send(res, 200, { protocol_version: 1, operations: [{ id: "onboarding.state" }], server: "stand-in" });
  if (url.pathname === "/api/v1/auth/status") return send(res, 200, { setup: false, signed_in: true });
  if (url.pathname === "/api/v1/auth/me") return send(res, 200, { user: { id: "acc-1", name: "Kay", username: "kay", role: "owner", capabilities: [] }, installation: "inst-1", email_setup_required: false });
  if (url.pathname === "/api/v1/live" || url.pathname === "/api/v1/account/live") { res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" }); res.write(": open\n\n"); sse.add(res); res.on("close", () => sse.delete(res)); return; }
  if (url.pathname === "/api/v1/operations" && req.method === "POST") {
    let body = ""; req.on("data", (d) => (body += d)); req.on("end", () => {
      const { request_id, operation, input } = JSON.parse(body);
      if (process.env.DEBUG) console.log("op:", operation, JSON.stringify(input).slice(0, 120));
      const ok = (data) => send(res, 200, { ok: true, data });
      if (applied.has(request_id)) return ok(applied.get(request_id)); // a repeat gets the first answer
      switch (operation) {
        case "spaces.list": return ok({ spaces: [{ id: SPACE, name: "Personal", kind: "personal", role: "owner", created_at: now() }] });
        case "notes.list": return ok({ notes: [{ id: note.id, space_id: SPACE, title: note.title, updated_at: note.updated_at, revision: note.revision }], next_cursor: null });
        case "notes.get": return ok({ note });
        case "tags.list": return ok({ tags: [], next_cursor: null });
        case "drafts.get": case "drafts.save": case "drafts.delete": return ok({ record: null, result: null });
        case "drafts.list": case "pending.list": return ok({ records: [], bookmark: "0" });
        case "onboarding.state": return ok({ finished: true });
        case "threads.list": return ok({ threads: [], next_cursor: null });
        case "requests.list": return ok({ requests: [], records: [] });
        case "threads.models": return ok({ models: [], providers: [] });
        case "collections.list": return ok({ items: [] });
        case "sharing.shared_with_me": return ok({ items: [] });
        case "notes.update": {
          seen.push(request_id);
          if (input.base_revision !== note.revision) return send(res, 409, { ok: false, error: { code: "conflict", message: "stale" } });
          if (input.title === "REFUSE") return send(res, 403, { ok: false, error: { code: "forbidden", message: "no" } });
          Object.assign(note, { title: input.title, markdown: input.markdown, revision: `r${Number(note.revision.slice(1)) + 1}`, updated_at: now() });
          const data = { note: { ...note } }; applied.set(request_id, data); return ok(data);
        }
        default: unhandled.add(operation); return ok({});
      }
    });
    return;
  }
  send(res, 404, { ok: false, error: { code: "not_found", message: "" } });
});
await new Promise((r) => server.listen(PORT, "127.0.0.1", r));
// Its own preview on a free port, with the proxy pointed at the stand-in (the shared one, when CAL_URL is set, proxies to somewhere else).
import { spawn } from "node:child_process";
import { DEVICES } from "./lib.mjs";
const WEB = await free(), BASE = `http://127.0.0.1:${WEB}`;
const preview = spawn("npx", ["vite", "preview", "--port", String(WEB), "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: true, env: { ...process.env, CAL_SERVER: `http://127.0.0.1:${PORT}` } });
const stop = () => { try { process.kill(-preview.pid); } catch { /* gone */ } };
process.on("exit", stop);
for (let i = 0; i < 120; i++) { try { if ((await fetch(BASE)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await chromium.launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const ctl = (mode) => fetch(`http://127.0.0.1:${PORT}/__away?mode=${mode ?? ""}`).then((r) => r.json());
const state = () => fetch(`http://127.0.0.1:${PORT}/__state`).then((r) => r.json());
try {
  for (const mode of ["502", "drop"]) {
    note.revision = "r1"; note.markdown = "# Journal replay notes\n\nThe journal is read twice.\n"; seen.length = 0; attempts.length = 0; applied.clear();
    const ctx = await b.newContext({ ...DEVICES.desktop });
    await ctx.addInitScript(([t]) => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.devtoken", t); localStorage.setItem("cal.server", "1"); } catch {} }, ["dev-token"]);
    const page = await ctx.newPage();
    if (process.env.DEBUG) { page.on("console", (m) => console.log("console:", m.type(), m.text().slice(0, 200))); page.on("pageerror", (e) => console.log("pageerror:", e.message.slice(0, 300))); page.on("requestfailed", (r) => console.log("requestfailed:", r.url())); }
    await page.goto(`${BASE}/notes/${NOTE}?server=1`); await page.waitForSelector("html[data-ready]");
    await page.locator(".cm-content").waitFor({ timeout: 20000 }).catch(async (e) => { console.log("page says:", (await page.evaluate(() => document.body.innerText)).slice(0, 300).replace(/\n/g, " | "), "unhandled:", [...unhandled].join(",")); throw e; });
    await page.getByText("Saved", { exact: true }).waitFor({ timeout: 10000 });
    await page.locator(".cm-content").click(); await page.keyboard.press("Control+End");
    await ctl(mode);                                    // the server goes away ...
    await page.keyboard.type(`\nWritten while the server was away (${mode}).`);
    await page.waitForTimeout(2600);                    // ... the save (after its 1.5 s pause) is tried and waits
    const banner = await page.getByText(/Can't reach the server/).count();
    const status = await page.locator(".state").textContent();
    ok(banner > 0, `${mode}: the reconnecting state is shown while the server is away`);
    ok(!/Not saved$/.test((status ?? "").trim()) || /waiting|Saving|Can't reach/.test(status ?? ""), `${mode}: the save line does not say Not saved while it waits`, JSON.stringify(status));
    await page.waitForTimeout(2400); await ctl(null);   // 5 s in all, then it is back
    await page.getByText("Saved", { exact: true }).waitFor({ timeout: 20000 });
    const st = await state();
    ok(st.applied.length === 1 && st.markdown.includes(`(${mode})`), `${mode}: the change reached the server exactly once`, `${st.applied.length} applied`);
    const ids = new Set([...st.attempts.map((a) => a[0]), ...st.seen]);
    ok(st.attempts.length >= 2 && ids.size === 1, `${mode}: the save was repeated while the server was away, every time with the original request id`, `${st.attempts.length} while away, ${ids.size} id`);
    ok((await page.getByText(/Can't reach the server/).count()) === 0, `${mode}: the reconnecting state goes once it answers`);
    await ctx.close();
  }
  // The server's own refusal is shown at once, not waited out.
  { note.revision = "r1"; seen.length = 0; attempts.length = 0; applied.clear();
    const ctx = await b.newContext({ ...DEVICES.desktop });
    await ctx.addInitScript(([t]) => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.devtoken", t); localStorage.setItem("cal.server", "1"); } catch {} }, ["dev-token"]);
    const page = await ctx.newPage();
    await page.goto(`${BASE}/notes/${NOTE}?server=1`); await page.waitForSelector("html[data-ready]");
    await page.locator(".cm-content").waitFor({ timeout: 20000 }); await page.getByText("Saved", { exact: true }).waitFor({ timeout: 10000 });
    await page.locator(".cm-content").click(); await page.keyboard.press("Control+Home"); await page.keyboard.press("Shift+End"); await page.keyboard.type("# REFUSE");
    for (let i = 0; i < 80 && (await state()).seen.length < 1; i++) await page.waitForTimeout(100);
    await page.waitForTimeout(2500); // a refusal that were retried would show here
    const t = Date.now();
    { const st = await state(); ok(st.seen.length === 1 && st.attempts.length === 0, "refused: the server's refusal is not retried (one attempt)", `${st.seen.length} reached, ${st.attempts.length} while away`); }
    await ctx.close(); }
  const un = (await state()).unhandled; if (un.length) console.log("INFO  operations the stand-in answered with {}:", un.join(", "));
} catch (e) { console.log("FAIL  " + e.message); bad++; }
await b.close(); stop(); server.close();
process.exit(bad ? 1 : 0);
