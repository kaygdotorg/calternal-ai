import { signUp } from "./lib.mjs";
// Seeds a disposable server for the scale measurements (e2e/scale-measure.mjs) through the product's own surfaces: the owner signs up with a virtual passkey; 10,000 notes carrying 2,000 distinct tags go in through
// `notes.create`; 49 more spaces through `spaces.create`; 199 more accounts join the first space by redeeming invitations in their own browser contexts (so 200 members); 100 live invitations stay open; 30 machines are
// enrolled from codes. Writes the owner's session to <dir>/state.json (cookie only). Env: CAL_BIN, CAL_DIST, CAL_SCALE_PORT, CAL_SCALE_DIR, SEED_NOTES/SEED_MEMBERS/SEED_MACHINES/SEED_TRASH (notes deleted into the Trash, default 1500) to shrink a run.
import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_SCALE_PORT ?? 3400), DIR = process.env.CAL_SCALE_DIR ?? "/tmp/cal-scale";
const NOTES = Number(process.env.SEED_NOTES ?? 10000), MEMBERS = Number(process.env.SEED_MEMBERS ?? 200), MACHINES = Number(process.env.SEED_MACHINES ?? 30), SPACES = Number(process.env.SEED_SPACES ?? 50), TAGS = 2000, INVITES = 100, TRASH = Number(process.env.SEED_TRASH ?? 1500);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
mkdirSync(DIR, { recursive: true });
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(DIR, "appdata"), "--userdata-ssd", join(DIR, "ssd"), "--userdata-hdd", join(DIR, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist", "--plugin-root", process.env.CAL_PLUGINS ?? "../../plugins"], { stdio: "ignore", detached: true });
srv.unref();
const url = `http://localhost:${PORT}`;
for (let i = 0; i < 120; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
const token = readFileSync(join(DIR, "appdata", "setup.token"), "utf8").trim();
const b = await chromium.launch();
const mk = async () => { const ctx = await b.newContext(); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1")); const p = await ctx.newPage(); const c = await ctx.newCDPSession(p); await c.send("WebAuthn.enable"); await c.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } }); return { ctx, p }; };
const t0 = Date.now(); const log = (m) => console.log(`[${Math.round((Date.now() - t0) / 1000)}s] ${m}`);
const { ctx, p } = await mk();
await p.goto(`${url}/?server=1`); await p.waitForSelector("html[data-ready]");
await signUp(p, token, "Scale Owner");
await p.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Set up"), null, { timeout: 20000 }); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);
const op = (operation, input) => p.evaluate(async ([operation, input]) => (await (await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation, input }) })).json()), [operation, input]);
const space = (await op("spaces.list", {})).data.spaces[0].id;
log("owner ready; space " + space.slice(0, 8));
// Notes: batches of 8 in parallel inside the page.
await p.evaluate(async ([space, n, tags, trash]) => {
  const post = (operation, input) => fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation, input }) }).then((r) => r.json());
  let next = 0, failed = 0;
  const worker = async () => { for (;;) { const i = next++; if (i >= n) return; const tag = `#t${i % tags}`; const r = await post("notes.create", { space_id: space, title: `Note ${String(i).padStart(5, "0")}`, markdown: `# Note ${String(i).padStart(5, "0")}\n\nSeeded note ${i} ${tag}\n` }); if (!r.ok) failed++; } };
  await Promise.all(Array.from({ length: 8 }, worker)); if (failed) throw new Error(`${failed} notes failed`);
  // The Trash: notes made and deleted at once, so it has more than one page.
  let gone = 0;
  const sweeper = async () => { for (;;) { const i = gone++; if (i >= trash) return; const r = await post("notes.create", { space_id: space, title: `Gone ${String(i).padStart(5, "0")}`, markdown: `# Gone ${i}\n` }); if (r.ok) await post("notes.delete", { id: r.data.note.id }); else failed++; } };
  await Promise.all(Array.from({ length: 8 }, sweeper)); if (failed) throw new Error(`${failed} notes failed`);
}, [space, NOTES, TAGS, TRASH]);
log(`${NOTES} notes with ${Math.min(TAGS, NOTES)} tags`);
for (let i = 1; i < SPACES; i++) await op("spaces.create", { name: `Space ${i}` });
log(`${SPACES} spaces`);
// Members: each joins by an invitation in its own context.
for (let i = 1; i < MEMBERS; i++) {
  const inv = await op("invites.create", { space_id: space, role: "editor" });
  let link, cursor;
  for (let page = 0; page < 30 && !link; page++) { const list = await op("invites.list", { space_id: space, ...(cursor ? { cursor } : {}) }); link = list.data.invites.find((x) => x.invite_id === inv.data.invite_id)?.link; cursor = list.data.next_cursor; if (!cursor) break; }
  if (!link) throw new Error(`no link for invitation ${i}`);
  await new Promise((r) => setTimeout(r, 3200)); // the server admits about 20 sign-ups a minute from one address
  let joined = false;
  for (let attempt = 0; attempt < 4 && !joined; attempt++) {
    const g = await mk();
    try {
      await g.p.goto(`${url}${new URL(link).pathname}`); await g.p.waitForSelector("html[data-ready]");
      await signUp(g.p, "", `Member ${String(i).padStart(3, "0")}`);
      await g.p.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Join"), null, { timeout: 20000 });
      joined = true;
    } catch (e) { log(`member ${i}: attempt ${attempt + 1} failed: ${(await g.p.locator('[role="alert"]').allInnerTexts().catch(() => [])).join(" ").slice(0, 120) || String(e.message).split("\n")[0]}`); await new Promise((r) => setTimeout(r, 20000)); }
    await g.ctx.close();
  }
  if (!joined) throw new Error(`member ${i} could not join`);
  if (i % 20 === 0) log(`${i + 1} members`);
}
for (let i = 0; i < INVITES; i++) await op("invites.create", { space_id: space, role: "reader" });
log(`${INVITES} open invitations`);
// Machines: codes from the page, enrolled by the CLI (not run: they show as offline).
for (let i = 0; i < MACHINES; i++) {
  const code = await p.evaluate(async () => (await (await fetch("/api/v1/machines/codes", { method: "POST", headers: { "content-type": "application/json" }, body: "{}" })).json()).code);
  await new Promise((res) => { const e = spawn(BIN, ["machine", "enroll", "--server", url, "--name", `machine-${String(i).padStart(2, "0")}`, "--dir", join(DIR, `m${i}`)], { env: { ...process.env, HOME: join(DIR, `home${i}`), CALTERNAL_ENROLL_CODE: code }, stdio: "ignore" }); e.on("exit", res); });
}
log(`${MACHINES} machines`);
writeFileSync(join(DIR, "state.json"), JSON.stringify({ cookies: (await ctx.storageState()).cookies, space }));
await b.close();
log("seeded");
