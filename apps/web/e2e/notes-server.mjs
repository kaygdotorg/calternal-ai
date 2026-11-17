// Notes on a real server (empty installation, virtual authenticator): create, type, saving states, rename by the heading, changes from elsewhere without polling, the conflict choice, offline,
// and a 10,000-line note: typing with the keystroke probe while saves run, and save round trips. Development-host numbers. Env: CAL_BIN, CAL_DIST, CAL_NOTES_PORT.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_NOTES_PORT ?? 3394);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-notes-"));
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist"], { stdio: "ignore" });
const url = `http://localhost:${PORT}`;
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s.length ? s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))] : 0; };
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await cdp.send("WebAuthn.enable"); await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await page.goto(`${url}/?server=1`); await page.waitForSelector("html[data-ready]");
  await page.fill('input[name="token"]', token); await page.fill('input[name="name"]', "Tester"); await page.getByRole("button", { name: "Create passkey" }).click();
  await page.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Set up"), null, { timeout: 15000 });
  const api = (op, input) => page.evaluate(async ([op, input]) => (await (await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: op, input }) })).json()), [op, input]);
  const space = (await api("spaces.list", {})).data.spaces[0].id;

  // Empty state, then the first note.
  await page.goto(`${url}/notes?server=1`); await page.waitForSelector("html[data-ready]");
  await page.getByRole("button", { name: "Write the first note" }).click();
  await page.waitForSelector(".cm-h1");
  ok(true, "empty Notes offers the first note, and it opens");
  const state = () => page.locator(".state").innerText();
  // Typing is saved: Not saved yet -> Saving -> Saved, and the server has the text.
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.keyboard.type("\n\nHello from the browser.");
  ok(/Not saved yet|Saving/.test(await state()), "typing shows Not saved yet or Saving");
  await page.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
  let list = (await api("notes.list", { space_id: space })).data.notes;
  const id = list[0].id;
  let note = (await api("notes.get", { space_id: space, id })).data.note;
  ok(/Hello from the browser/.test(note.markdown), "the server has the saved text");
  await page.waitForTimeout(800);
  const drafts = await api("drafts.list", {});
  console.log("drafts after save:", JSON.stringify(drafts).slice(0, 300));
  ok(!(drafts.data?.records ?? []).length && !(await page.getByText("unsent draft").isVisible()), "after a save there is no unsent draft left, on the server or on screen");
  // Rename: the heading is the title.
  await page.locator(".cm-h1").first().click(); await page.keyboard.press("End"); await page.keyboard.type(" renamed");
  await page.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
  note = (await api("notes.get", { space_id: space, id })).data.note;
  ok(/renamed/.test(note.title), "editing the heading renames the note", note.title);
  // A change from elsewhere shows without polling (nothing typed).
  const t0 = Date.now();
  await api("notes.update", { space_id: space, id, base_revision: note.revision, title: note.title, markdown: note.markdown + "\n\nAdded elsewhere." });
  await page.getByText("Added elsewhere.").waitFor({ timeout: 6000 });
  ok(true, "a change from elsewhere appears by itself", `${Date.now() - t0} ms`);
  // The conflict: type here, change there, then the save meets it. Keep mine.
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.keyboard.type(" Mine.");
  note = (await api("notes.get", { space_id: space, id })).data.note;
  await api("notes.update", { space_id: space, id, base_revision: note.revision, title: note.title, markdown: note.markdown + "\n\nTheirs again." });
  await page.getByText(/Someone changed this note/).waitFor({ timeout: 8000 });
  ok(/Mine\./.test(await page.locator(".cm-content").innerText()), "conflict: what was typed is kept on screen");
  await page.getByRole("button", { name: "Keep mine" }).click();
  await page.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
  note = (await api("notes.get", { space_id: space, id })).data.note;
  ok(/Mine\./.test(note.markdown), "keep mine: the server has the typed text");
  // Offline: typing is kept, the state says so, and nothing is lost.
  await ctx.setOffline(true);
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.keyboard.type(" Offline text.");
  await page.getByText(/Can't reach the server. Your text is still here/).first().waitFor({ timeout: 10000 });
  ok(true, "offline: the state says the text is still here and saves when the server is back", await state());
  ok(/Offline text\./.test(await page.locator(".cm-content").innerText()), "offline: the typed text is still on screen");
  await ctx.setOffline(false);
  await page.getByText("Saved", { exact: true }).waitFor({ timeout: 12000 });
  note = (await api("notes.get", { space_id: space, id })).data.note;
  ok(/Offline text\./.test(note.markdown), "back online: the text typed offline is saved", "");

  // A note that is not there says so and offers the way back.
  await page.goto(`${url}/notes/00000000-0000-7000-8000-000000000000?server=1`); await page.waitForSelector("html[data-ready]");
  await page.getByRole("heading", { name: /Note not found|You can't open/ }).waitFor({ timeout: 8000 });
  ok(await page.getByRole("button", { name: "Try again" }).isVisible(), "a missing note says so and offers Try again");
  // 10,000 lines: make it through the API, open it in the UI, type with the probe while saves run.
  const lines = Array.from({ length: 10000 }, (_, i) => i % 50 === 0 ? `## Section ${i / 50}` : `Line ${i}: the quick brown fox jumps over the lazy dog, then naps in the sun.`).join("\n");
  const big = (await api("notes.create", { space_id: space, title: "Big", markdown: `# Big\n\n${lines}\n` })).data.note;
  await page.goto(`${url}/notes/${big.id}?server=1&test=1`); await page.waitForSelector("html[data-ready]");
  await page.waitForSelector(".cm-h1", { timeout: 20000 }); await page.waitForTimeout(1500);
  await page.evaluate(() => { performance.clearResourceTimings(); window.__keys = []; });
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+Home");
  for (let i = 0; i < 25; i++) await page.keyboard.press("PageDown");
  await page.evaluate(() => { window.__keys = []; });
  for (let round = 0; round < 3; round++) { await page.keyboard.type("the quick brown fox jumps over the lazy dog. ", { delay: 50 }); await page.waitForTimeout(1900); } // a save fires after each pause
  await page.getByText("Saved", { exact: true }).waitFor({ timeout: 15000 });
  const keys = await page.evaluate(() => window.__keys ?? []);
  const saves = await page.evaluate(() => performance.getEntriesByType("resource").filter((r) => r.name.endsWith("/api/v1/operations") && r.initiatorType === "fetch").map((r) => r.duration));
  console.log(`typing in a 10,000-line note with saves running: ${keys.length} keys, median ${pct(keys, 0.5).toFixed(1)} ms, p95 ${pct(keys, 0.95).toFixed(1)} ms, worst ${Math.max(0, ...keys).toFixed(1)} ms`);
  console.log(`operations round trips seen by the page (saves, list refreshes, draft writes): n=${saves.length}, median ${pct(saves, 0.5).toFixed(0)} ms, p95 ${pct(saves, 0.95).toFixed(0)} ms, max ${Math.max(0, ...saves).toFixed(0)} ms`);
  ok(keys.length > 100, "the probe saw the keystrokes");
  ok(pct(keys, 0.95) <= 16 * 2, "typing while saving: p95 keystroke to paint within two frames (development host)", `${pct(keys, 0.95).toFixed(1)} ms`);
  await b.close();
} catch (e) { console.error(e); bad++; }
srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);
