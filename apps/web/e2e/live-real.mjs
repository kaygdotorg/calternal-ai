// Live rooms against a real server built from this tree: two browsers (two accounts, virtual passkeys) on one note, typing both ways, presence, a peer leaving, access revoked mid-edit with the rescue
// holding only the local words, and a reconnect after the server restarts. Each scenario prints PASS or FAIL with the lane the failure belongs to; a failing scenario does not stop the next.
// Usage: CAL_BIN=<calternal binary> CAL_DIST=<apps/web/dist> node e2e/live-real.mjs   (dev-01; own disposable roots under $TMPDIR; prints no secret)
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { signUp } from "./lib.mjs";
const here = dirname(fileURLToPath(import.meta.url));
const BIN = process.env.CAL_BIN, DIST = process.env.CAL_DIST ?? "dist", PLUGINS = process.env.CAL_PLUGINS ?? resolve(here, "..", "..", "..", "plugins");
if (!BIN) { console.error("CAL_BIN (the calternal binary) is required"); process.exit(2); }
const free = () => new Promise((res) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); });
const PORT = await free(), url = `http://localhost:${PORT}`, root = mkdtempSync(join(tmpdir(), "cal-live-")), INSTALL = crypto.randomUUID();
const args = (create) => ["serve", ...(create ? ["--create"] : []), "--installation-id", INSTALL, "--appdata", join(root, "appdata"), "--userdata-ssd", join(root, "ssd"), "--userdata-hdd", join(root, "hdd"), "--port", String(PORT), "--origin", url, "--web-dir", DIST, "--plugin-root", PLUGINS];
let srv = null;
const start = async (create) => { srv = spawn(BIN, args(create), { stdio: ["ignore", "ignore", process.env.DEBUG ? "inherit" : "ignore"] }); for (let i = 0; i < 120; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) return; } catch {} await new Promise((r) => setTimeout(r, 250)); } throw new Error("the server did not start"); };
const stopServer = async () => { srv.kill("SIGTERM"); await new Promise((r) => { srv.once("exit", r); setTimeout(r, 5000); }); };
let bad = 0;
const ok = (c, m, lane = "", note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${c || !lane ? "" : `   [lane: ${lane}]`}${note ? "  " + note : ""}`); if (!c) bad++; };
const scenario = async (name, lane, f) => { try { await f(); } catch (e) { ok(false, `${name}: ${String(e.message).split("\n")[0]}`, lane); } };
const op = (p, operation, input = {}) => p.evaluate(async ([operation, input]) => (await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation, input }) })).json(), [operation, input]);
const text = (p) => p.locator(".cm-content").innerText();
const until = async (f, ms = 12000) => { const t = Date.now(); for (;;) { try { const v = await f(); if (v) return v; } catch {} if (Date.now() - t > ms) return false; await new Promise((r) => setTimeout(r, 150)); } };
const b = await chromium.launch();
const mk = async (name) => { const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => { localStorage.setItem("cal.firstrun", "1"); }); const p = await ctx.newPage(); p.on("dialog", (d) => void d.accept()); if (process.env.DEBUG) { p.on("console", (m) => { if (/live|room|socket|WebSocket/i.test(m.text())) console.log(`[${name}]`, m.text().slice(0, 160)); }); p.on("pageerror", (e) => console.log(`[${name}] pageerror`, e.message.slice(0, 200))); } const c = await ctx.newCDPSession(p); await c.send("WebAuthn.enable"); await c.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } }); return { ctx, p, name }; };
try {
  await start(true);
  const token = readFileSync(join(root, "appdata", "setup.token"), "utf8").trim();
  const A = await mk("owner"), G = await mk("guest");
  await A.p.goto(`${url}/?server=1`); await A.p.waitForSelector("html[data-ready]");
  await signUp(A.p, token, "Olive");
  await A.p.waitForSelector("html[data-ready]");
  // An invitation for the second person.
  await A.p.evaluate(() => { history.pushState(null, "", "/settings/invites"); dispatchEvent(new PopStateEvent("popstate")); });
  await A.p.getByRole("button", { name: "Make an invitation" }).click(); await A.p.locator("code.cmd").first().waitFor({ timeout: 10000 });
  const link = new URL((await A.p.locator("code.cmd").first().innerText()).trim());
  await G.p.goto(`${url}${link.pathname}`); await G.p.waitForSelector("html[data-ready]");
  await signUp(G.p, "", "Gus");
  const me = async (P) => (await P.evaluate(async () => (await fetch("/api/v1/auth/me")).json())).user;
  const [olive, gus] = [await me(A.p), await me(G.p)];
  const spaceOf = async (P) => (await op(P, "spaces.list")).data.spaces[0].id;
  const space = await spaceOf(A.p), gusSpace = await spaceOf(G.p);
  const made = await op(A.p, "notes.create", { space_id: space, title: "Plan", markdown: "# Plan\n\nStart here.\n" });
  const note = made.data?.note; ok(!!note, "setup: an owner, a second account and a note in the owner's space", "backend (accounts/notes)", JSON.stringify(made.error ?? ""));
  const grant = await op(A.p, "sharing.grant", { item: { kind: "notes", id: note.id }, audience: { account_id: gus.id }, permission: "edit" });
  ok(grant.ok, "setup: Edit on the note granted to the second account", "sharing backend", JSON.stringify(grant.error ?? ""));
  const open = async (P, id = note.id) => { await P.p.goto(`${url}/notes/${id}?server=1&live=1`); await P.p.waitForSelector("html[data-ready]"); await P.p.locator(".cm-content").waitFor({ timeout: 20000 }); };
  const saved = (P) => P.p.getByText("Saved", { exact: true }).waitFor({ timeout: 15000 });

  await scenario("typing", "live-rooms backend or ui-notes", async () => {
    await open(A); await open(G);
    await saved(A); await saved(G);
    await A.p.locator(".cm-content").click(); await A.p.keyboard.press("Control+End"); await A.p.keyboard.type("\nOWNER-ONE");
    ok(await until(async () => (await text(G.p)).includes("OWNER-ONE")), "typing: what the owner types reaches the guest", "live-rooms backend (room) or ui-notes (binding)");
    await G.p.locator(".cm-content").click(); await G.p.keyboard.press("Control+End"); await G.p.keyboard.type("\nGUEST-ONE");
    ok(await until(async () => (await text(A.p)).includes("GUEST-ONE")), "typing: what the guest types reaches the owner", "live-rooms backend (room) or ui-notes (binding)");
    ok(await until(async () => (await text(A.p)) === (await text(G.p))), "typing: both end with the same text");
    await saved(A); await saved(G);
    ok(true, "typing: both say Saved once the room confirmed");
    const got = await op(A.p, "notes.get", { space_id: space, id: note.id });
    ok(/OWNER-ONE/.test(got.data?.note?.markdown ?? "") && /GUEST-ONE/.test(got.data?.note?.markdown ?? ""), "typing: the saved note holds both people's words", "live-rooms backend (durability)", (got.data?.note?.markdown ?? "").slice(-40).replace(/\n/g, "|"));
  });
  await scenario("presence", "ui-notes (names) or live-rooms backend (awareness)", async () => {
    ok(await until(() => A.p.locator("[data-presence]").count().then((n) => n > 0)), "presence: the owner sees the guest's presence in the header", "live-rooms backend (awareness) or ui-notes");
    ok(await until(() => G.p.locator("[data-presence]").count().then((n) => n > 0)), "presence: the guest sees the owner's presence in the header", "live-rooms backend (awareness) or ui-notes");
    const label = await A.p.locator("[data-presence]").first().getAttribute("aria-label");
    ok(/Gus/.test(label ?? ""), "presence: the guest is named, not 'Someone'", "ui-notes (name lookup for a person outside the space)", JSON.stringify(label));
    await G.p.locator(".cm-content").click(); await G.p.keyboard.press("Control+Home");
    ok(await until(() => A.p.locator(".cm-ySelectionCaret").count().then((n) => n > 0)), "presence: the owner sees the guest's cursor in the text", "live-rooms backend (cursor in awareness) or ui-notes");
  });
  await scenario("a peer leaving", "live-rooms backend (awareness removal)", async () => {
    await G.p.goto("about:blank");
    ok(await until(() => A.p.locator("[data-presence]").count().then((n) => n === 0), 20000), "a peer leaving: the owner's presence strip empties after the guest closes the note", "live-rooms backend (clear on leave)");
    await open(G); await saved(G);
  });
  await scenario("revoked mid-edit", "live-rooms backend (access_revoked) or ui-notes (rescue)", async () => {
    await G.p.locator(".cm-content").click(); await G.p.keyboard.press("Control+End");
    await G.p.context().setOffline(true);
    await G.p.keyboard.type("\nGUEST-LOCAL-WORDS");
    await A.p.locator(".cm-content").click(); await A.p.keyboard.press("Control+End"); await A.p.keyboard.type("\nOWNER-LATER-WORDS");
    const list = await op(A.p, "sharing.list", { item: { kind: "notes", id: note.id } });
    const gid = (list.data?.grants ?? []).find((g) => g.audience?.account_id === gus.id)?.grant_id;
    const rev = await op(A.p, "sharing.revoke", { item: { kind: "notes", id: note.id }, grant_id: gid });
    ok(rev.ok, "revoked: the owner takes the guest's access away", "sharing backend", JSON.stringify(rev.error ?? ""));
    await G.p.context().setOffline(false);
    const line = await until(async () => (await G.p.locator("p.err[role=alert]").allInnerTexts()).find((t) => /no longer open|kept/.test(t)), 20000);
    ok(!!line, "revoked: the guest is told in one line what happened to their edits", "ui-notes or live-rooms backend (the frame after a reconnect)", JSON.stringify(line || ""));
    const rescued = await op(G.p, "notes.list", { space_id: gusSpace });
    const copy = (rescued.data?.notes ?? []).find((n) => /Your edits to/.test(n.title));
    const body = copy ? (await op(G.p, "notes.get", { space_id: gusSpace, id: copy.id })).data?.note?.markdown ?? "" : "";
    ok(!!copy && body.includes("GUEST-LOCAL-WORDS") && !body.includes("OWNER-LATER-WORDS"), "revoked: the rescue note in the guest's own space holds the guest's words and none of the owner's", "ui-notes (intent) or sharing backend (rescue)", copy ? JSON.stringify(body.slice(0, 80)) : "no rescue note");
  });
  await scenario("server restart", "live-rooms backend (restart/reconnect) or ui-notes (client retry)", async () => {
    await open(A); await saved(A);
    await A.p.locator(".cm-content").click(); await A.p.keyboard.press("Control+End"); await A.p.keyboard.type("\nBEFORE-RESTART");
    await saved(A);
    await stopServer();
    await A.p.keyboard.type("\nDURING-OUTAGE");
    await new Promise((r) => setTimeout(r, 4000));
    await start(false);
    ok(await until(async () => (await op(A.p, "notes.get", { space_id: space, id: note.id })).data?.note?.markdown?.includes("DURING-OUTAGE"), 45000), "restart: text typed while the server was down reaches it after it restarts, without a reload", "ui-notes (offline replay) or live-rooms backend (rejoin)");
    await saved(A).catch(() => {});
    ok(await A.p.getByText("Saved", { exact: true }).count() > 0, "restart: the page says Saved again", "ui-notes");
    const before = (await text(A.p)).length;
    await A.p.keyboard.type("\nAFTER-RESTART");
    ok(await until(async () => (await op(A.p, "notes.get", { space_id: space, id: note.id })).data?.note?.markdown?.includes("AFTER-RESTART"), 20000), "restart: typing after the restart is saved", "live-rooms backend or ui-notes", `${before} chars before`);
  });
} catch (e) { console.log("FAIL  harness: " + String(e.message).split("\n")[0]); bad++; }
await b.close(); if (srv) srv.kill("SIGTERM"); rmSync(root, { recursive: true, force: true });
process.exit(bad ? 1 : 0);
