import { signUp } from "./lib.mjs";
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
  if (process.env.HACK_TYPING) await ctx.addInitScript(() => { const f = window.addEventListener; window.addEventListener = function (t, fn, o) { if (t === "keydown" && String(fn).includes("isComposing")) return; return f.call(this, t, fn, o); }; });
  await page.goto(`${url}/?server=1`); await page.waitForSelector("html[data-ready]");
  await signUp(page, token, "Tester");
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
  const ring = await page.evaluate(() => ({ mode: document.documentElement.dataset.input, outline: getComputedStyle(document.querySelector(".cm-content")).outlineStyle }));
  ok(ring.mode === "pointer" && ring.outline === "none", "typing in the editor draws no focus outline (keys typed in a field do not switch to keyboard mode)", JSON.stringify(ring));
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

  // Tags, pickers, rename from the list, Trash with Undo and Restore.
  if (!process.env.SKIP_LISTS) {
  const tagged = (await api("notes.create", { space_id: space, title: "Planning", markdown: "# Planning\n\nQuarter goals #work/plans and #home\n" })).data.note;
  await page.goto(`${url}/notes?server=1`); await page.waitForSelector("html[data-ready]");
  await page.getByRole("button", { name: /^#work/ }).first().waitFor({ timeout: 8000 });
  ok(true, "the tag list shows the space's tags");
  { let n = 0; const on = (r) => r.url().includes("/api/v1/operations") && n++; page.on("request", on); await page.waitForTimeout(2000); page.off("request", on); ok(n < 6, "an idle Notes page makes no request storm", `${n} requests in 2 s`); }
  await page.getByRole("button", { name: /^#work/ }).first().evaluate((el) => el.click()); // (the chips sit under the sidebar's header while it is scrolled; a script click does not care)
  await page.waitForTimeout(600);
  ok((await page.locator('nav[aria-label="Notes"] a.row').count()) === 1, "filtering by a tag lists only its notes");
  await page.getByRole("button", { name: "All", exact: true }).evaluate((el) => el.click()); await page.waitForTimeout(500);
  await page.locator('nav[aria-label="Notes"] a.row', { hasText: "Planning" }).click(); await page.waitForSelector(".cm-h1");
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.keyboard.type("\nSee [[Unt");
  await page.getByRole("option", { name: /Untitled/ }).first().waitFor({ timeout: 6000 });
  ok(true, "the [[ picker finds notes from the server by title prefix");
  await page.keyboard.press("Enter"); await page.waitForTimeout(300);
  ok(/\[Untitled[^\]]*\]\(</.test(await page.locator(".cm-content").innerText()) || /Untitled/.test(await page.locator(".cm-content").innerText()), "choosing it inserts a link to the note");
  await page.keyboard.type(" #wo"); await page.getByRole("option", { name: /#work/ }).first().waitFor({ timeout: 6000 });
  ok(true, "the # picker finds tags from the server");
  await page.keyboard.press("Escape");
  await page.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
  // Rename from the list.
  const row = page.locator('nav[aria-label="Notes"] [role=listitem]', { hasText: "Planning" });
  await row.getByRole("button", { name: /Actions for/ }).click(); await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.fill('input[aria-label="Note title"]', "Quarter plan"); await page.keyboard.press("Enter");
  await page.locator('nav[aria-label="Notes"] a.row', { hasText: "Quarter plan" }).waitFor({ timeout: 6000 });
  ok(true, "rename from the list");
  // Trash with Undo.
  await page.locator('nav[aria-label="Notes"] [role=listitem]', { hasText: "Quarter plan" }).getByRole("button", { name: /Actions for/ }).click(); await page.getByRole("menuitem", { name: "Move to Trash" }).click();
  await page.getByText(/Moved .* to the Trash/).first().waitFor({ timeout: 6000 });
  ok((await page.locator('nav[aria-label="Notes"] a.row', { hasText: "Quarter plan" }).count()) === 0, "delete moves the note out of the list");
  await page.getByRole("button", { name: "Undo" }).click(); await page.locator('nav[aria-label="Notes"] a.row', { hasText: "Quarter plan" }).waitFor({ timeout: 6000 });
  ok(true, "Undo brings it back");
  // Trash view and Restore.
  await page.locator('nav[aria-label="Notes"] [role=listitem]', { hasText: "Quarter plan" }).getByRole("button", { name: /Actions for/ }).click(); await page.getByRole("menuitem", { name: "Move to Trash" }).click();
  await page.getByRole("link", { name: "Trash" }).click(); await page.getByRole("heading", { name: "Trash" }).waitFor();
  await page.locator("article .t", { hasText: "Quarter plan" }).waitFor({ timeout: 6000 });
  await page.getByRole("button", { name: "Restore" }).click(); await page.waitForTimeout(800);
  ok((await page.locator("article .t", { hasText: "Quarter plan" }).count()) === 0, "Restore takes it out of the Trash");
  await page.goto(`${url}/notes?server=1`); await page.locator('nav[aria-label="Notes"] a.row', { hasText: "Quarter plan" }).waitFor({ timeout: 6000 });
  ok(true, "the restored note is back in the list");
  }
  // The palette reaches every space and every note of the space: switch space from it, and find a note by title.
  await api("spaces.create", { name: "Elsewhere" });
  await page.goto(`${url}/notes?server=1`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
  await page.keyboard.press("Control+KeyK"); await page.getByRole("dialog").first().waitFor({ timeout: 4000 });
  await page.keyboard.type("Elsewhere"); await page.waitForTimeout(700);
  await page.getByRole("dialog").getByText(/Elsewhere/).first().waitFor({ timeout: 4000 });
  await page.keyboard.press("Enter"); await page.waitForTimeout(800);
  ok(true, "the palette lists a space to switch to");
  ok((await page.evaluate(() => localStorage.getItem("cal.space"))) !== space, "choosing it makes that space current");
  await page.evaluate((id) => { localStorage.setItem("cal.space", id); }, space);
  await page.goto(`${url}/notes?server=1`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
  await page.keyboard.press("Control+KeyK"); await page.getByRole("dialog").first().waitFor({ timeout: 4000 });
  await page.keyboard.type("Quarte"); await page.getByRole("dialog").getByText(/Quarter plan/).first().waitFor({ timeout: 6000 });
  ok(true, "the palette finds a note of the space by the start of its title");
  await page.keyboard.press("Escape");
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
  if (process.env.HACK_BLINK) await page.addStyleTag({ content: ".cm-cursorLayer, .cm-cursorLayer * { animation: none !important }" });
  if (process.env.HACK_ANIM) await page.addStyleTag({ content: "*, *::before, *::after { animation-duration: 0s !important; transition-duration: 0s !important }" });
  if (process.env.HACK_KEY) await page.evaluate(() => { document.documentElement.dataset.input = "key"; });
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
