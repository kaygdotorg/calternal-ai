import { signUp } from "./lib.mjs";
// The pickers and message search against a real server of this tree: the composer's @ lists the account from people.lookup, / lists skills.list, @ with a typed title finds a thread by threads.lookup, and message search (search.query) opens a thread.
// Same setup as readiness-server.mjs. Env: CAL_BIN, CAL_DIST, CAL_LOOKUP_PORT.
import { spawn } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_LOOKUP_PORT ?? 3414);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-threads/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-lookup-"));
const url = `http://localhost:${PORT}`;
let bad = 0, machine = null;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist", "--plugin-root", process.env.CAL_PLUGINS ?? "../../plugins"], { stdio: "ignore" });
const op = (p, name, input) => p.evaluate(async ([name, input]) => { const r = await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: name, input }) }); return r.json(); }, [name, input]);
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const p = await ctx.newPage(); const cdp = await ctx.newCDPSession(p);
  await cdp.send("WebAuthn.enable"); await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await p.goto(`${url}/?server=1`); await p.waitForSelector("html[data-ready]");
  await signUp(p, token, "Lookup Owner");
  await p.waitForTimeout(2500); await p.evaluate(() => localStorage.removeItem("cal.onboarding")); /* the first owner lands in the onboarding (shell lane): the account exists, leave the wizard */ await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
  const space = (await op(p, "spaces.list", {})).data.spaces.find((s) => s.personal).id;

  const t1 = (await op(p, "threads.create", { space_id: space, title: "Journal replay plan" })).data.thread;
  const t2 = (await op(p, "threads.create", { space_id: space, title: "Storage layout review" })).data.thread;
  const sent = await op(p, "threads.send", { space_id: space, id: t2.id, message: "The journal replay storage layout needs a review" });
  console.log("   threads.send:", JSON.stringify(sent).slice(0, 160));
  await p.goto(`${url}/threads/${t1.id}?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2000);
  // what the wire says, polled while the index catches up
  let wireLookup = null; for (let i = 0; i < 30; i++) { wireLookup = (await op(p, "threads.lookup", { prefix: "Stor" })); if ((wireLookup.data?.hits ?? []).length) break; await p.waitForTimeout(500); }
  console.log("   threads.lookup:", JSON.stringify(wireLookup).slice(0, 300));
  console.log("   search.query hit:", JSON.stringify((await op(p, "search.query", { query: "journal", kinds: ["threads"] })).data?.hits?.[0] ?? null));
  console.log("   skills.list:", JSON.stringify(await op(p, "skills.list", { space_id: space, prefix: "" })).slice(0, 200));
  const composer = p.locator("#composer-field");
  await composer.click(); await p.keyboard.type("@");
  const rows = p.locator("[role=listbox] [role=option]");
  for (let i = 0; i < 20 && !(await p.locator("[role=listbox]").count()); i++) await p.waitForTimeout(150);
  await p.waitForTimeout(500);
  const listed = (await p.locator("[role=listbox]").innerText().catch(() => "")).replace(/\s+/g, " ");
  ok(/Lookup Owner/.test(listed), "@ lists the account from people.lookup", listed.slice(0, 120));
  ok(!/Priya|Sam\b/.test(listed), "and none of the fixture people");
  await p.keyboard.type("Stor"); await p.waitForTimeout(900);
  const typed = (await p.locator("[role=listbox]").innerText().catch(() => "")).replace(/\s+/g, " ");
  const indexed = (wireLookup.data?.hits ?? []).length > 0;
  ok(indexed && /Storage layout review/.test(typed), "@Stor finds the thread by title prefix (threads.lookup)", typed.slice(0, 120));
  await p.keyboard.press("Escape"); await composer.fill("/"); await p.waitForTimeout(900);
  console.log("   / list:", (await p.locator("[role=listbox]").innerText().catch(() => "(no list)")).replace(/\s+/g, " ").slice(0, 120));
  await composer.fill(""); await p.keyboard.press("Escape");
  // message search over search.query: the index may still be catching up, so ask for a while
  await p.keyboard.press("Control+Shift+F");
  await p.locator(".msg-search-input").waitFor({ timeout: 3000 });
  await p.keyboard.type("journal");
  let hit = 0; for (let i = 0; i < 40 && !hit; i++) { await p.waitForTimeout(400); hit = await p.locator("[role=listbox][aria-label='Matching messages'] li").count(); }
  ok(hit > 0, "message search returns the message from search.query", String(hit));
  ok((await p.locator("[role=listbox][aria-label=\'Matching messages\'] li .w").count()) > 0, "and each hit shows when the message was written");
  if (hit) { await p.keyboard.press("Enter"); await p.waitForTimeout(800); ok(/\/threads\/[^/?]+/.test(p.url()) && (await p.locator(".msg-search-input").count()) === 0, "Enter opens it and closes the panel", p.url()); }
  await b.close();
} catch (e) { console.error(e); bad++; }
srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);
