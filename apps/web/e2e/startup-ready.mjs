import { signUp } from "./lib.mjs";
// Sign-up, first load and a warm reload against a real server: after each, Threads AND Notes are READY: a thread and a note made through the API are listed in their sidebars, a thread opens with its composer, a note opens with its editor (a held or never-issued call would leave one of these empty: the hold on a mis-guessed "write" once kept the Threads store from ever becoming ready). No timings, only that they happen. Env: CAL_BIN, CAL_DIST, CAL_READY_UP_PORT.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_READY_UP_PORT ?? 3416);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-threads/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-readyup-"));
const url = `http://localhost:${PORT}`;
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist", "--plugin-root", process.env.CAL_PLUGINS ?? "../../plugins"], { stdio: "ignore" });
const op = (p, name, input) => p.evaluate(async ([name, input]) => { const r = await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: name, input }) }); return r.json(); }, [name, input]);
/** Threads and Notes are ready on this page: both lists show what the API holds, and each opens. `held` is what the page asked and never got an answer to. */
async function ready(p, label, thread, note) {
  const pending = new Set(), born = new Map();
  p.on("request", (r) => { if (r.url().includes("/api/v1/operations")) { pending.add(r); born.set(r, Date.now()); } }); p.on("requestfinished", (r) => pending.delete(r)); p.on("requestfailed", (r) => pending.delete(r));
  await p.goto(`${url}/threads?server=1`); await p.waitForSelector("html[data-ready]");
  await p.getByText(thread.title).first().waitFor({ timeout: 20000 }).catch(() => {});
  ok(await p.getByText(thread.title).count() >= 1, `${label}: Threads is ready (the thread made through the API is listed)`);
  await p.goto(`${url}/threads/${thread.id}?server=1`); await p.waitForSelector("html[data-ready]");
  await p.locator("#composer-field").waitFor({ timeout: 20000 }).catch(() => {});
  ok(await p.locator("#composer-field").count() === 1, `${label}: the thread opens with its composer`);
  await p.goto(`${url}/notes?server=1`); await p.waitForSelector("html[data-ready]");
  await p.locator("nav[aria-label=Notes]").getByText(note.title).first().waitFor({ timeout: 20000 }).catch(() => {});
  ok(await p.locator("nav[aria-label=Notes]").getByText(note.title).count() >= 1, `${label}: Notes is ready (the note made through the API is listed)`);
  await p.goto(`${url}/notes/${note.id}?server=1`); await p.waitForSelector("html[data-ready]");
  await p.locator(".cm-content").waitFor({ timeout: 20000 }).catch(() => {});
  ok(await p.locator(".cm-content").count() >= 1, `${label}: the note opens with its editor`);
  for (let i = 0; i < 30 && pending.size; i++) await p.waitForTimeout(500); // a busy machine answers late; an answer that never comes is what this looks for
  ok(pending.size === 0, `${label}: no operation is left unanswered`, [...pending].map((r) => `${/"operation":"([^"]+)"/.exec(r.postData() ?? "")?.[1]}@${Math.round((Date.now() - born.get(r)) / 1000)}s`).join(","));
}
try {
  for (let i = 0; i < 120; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const p = await ctx.newPage(); const cdp = await ctx.newCDPSession(p);
  await cdp.send("WebAuthn.enable"); await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await p.goto(`${url}/?server=1`); await p.waitForSelector("html[data-ready]");
  await signUp(p, token, "Ready Owner");
  await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);
  const space = (await op(p, "spaces.list", {})).data.spaces.find((s) => s.personal).id;
  const thread = { ...(await op(p, "threads.create", { space_id: space, title: "Ready thread" })).data.thread, title: "Ready thread" };
  const note = { ...(await op(p, "notes.create", { space_id: space, title: "Ready note", markdown: "# Ready note\n\nBody\n" })).data.note, title: "Ready note" };
  ok(!!thread?.id && !!note?.id, "sign-up made an account with a space; a thread and a note were created through the API");
  await ready(p, "first load after sign-up", thread, note);
  const warm = await ctx.newPage();
  await ready(warm, "warm reload (a second page of the same browser)", thread, note);
  await warm.reload({ waitUntil: "domcontentloaded" }); await warm.waitForSelector("html[data-ready]");
  await b.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
finally { srv.kill(); rmSync(data, { recursive: true, force: true }); }
process.exit(bad ? 1 : 0);
