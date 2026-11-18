// A note in a room (fixture room, ?live=1): it opens, another person's presence shows in the header and their cursor in the text, their typing arrives, this person's undo takes back only this person's own typing, and
// the save line says Saved once the room has confirmed. Desktop and phone. Frames go to $CAL_SHOTS (default /tmp/calternal-share-shots), never into the repository.
// Usage: node e2e/with-preview.mjs e2e/live.mjs
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";
const SHOTS = process.env.CAL_SHOTS ?? "/tmp/calternal-share-shots"; mkdirSync(SHOTS, { recursive: true });
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (const device of ["desktop", "phone"]) {
    const { page: p, ctx } = await open(browser, device, "light", "/notes/n-1?firstrun=0&live=1");
    await p.locator(".cm-content").waitFor({ timeout: 15000 });
    await p.getByRole("img", { name: /Here now: Maya Okafor/ }).waitFor({ timeout: 8000 });
    ok(true, `${device}: the other person is shown in the header, by name`);
    await p.locator(".cm-ySelectionCaret").first().waitFor({ timeout: 8000 });
    ok(true, `${device}: their cursor is drawn in the text`);
    await p.getByText("(Maya: checked)").waitFor({ timeout: 8000 });
    ok(true, `${device}: what they type arrives`);
    // My own typing, then undo: only mine goes.
    await p.locator(".cm-content").click(); await p.keyboard.press("Control+End");
    await p.keyboard.type(" mine");
    await p.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
    ok(true, `${device}: the save line says Saved once the room confirmed`);
    await p.keyboard.press("Control+z");
    const text = await p.locator(".cm-content").innerText();
    ok(!text.includes(" mine") && text.includes("(Maya: checked)"), `${device}: undo takes back this person's typing and not the other person's`);
    await p.screenshot({ path: `${SHOTS}/live-${device}.png` });
    await ctx.close();
  }
  // The socket drops mid-edit, the page reloads, the room comes back: nothing typed is lost, and the room has it afterwards.
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/notes/n-2?firstrun=0&live=1&peer=0&test=1");
    await p.locator(".cm-content").waitFor({ timeout: 15000 });
    await p.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
    await p.evaluate(() => window.__net.cut());
    await p.locator(".cm-content").click(); await p.keyboard.press("Control+End"); await p.keyboard.type(" typed while cut off");
    await p.waitForFunction(() => /Not saved|Can.t reach|Offline|waiting/i.test(document.querySelector(".state")?.textContent ?? ""), null, { timeout: 8000 });
    ok(true, "offline: with the room unreachable the line says the text is not saved yet");
    await p.waitForTimeout(600);
    await p.reload(); await p.locator(".cm-content").waitFor({ timeout: 15000 });
    await p.waitForFunction(() => document.querySelector(".cm-content")?.textContent.includes("typed while cut off"), null, { timeout: 12000 });
    ok(true, "offline: after a reload the text typed while cut off is still there (replayed from this device's copy)");
    await p.getByText("Saved", { exact: true }).waitFor({ timeout: 12000 });
    ok(true, "offline: and once the room is back it says Saved");
    await p.waitForTimeout(800);
    await p.reload(); await p.locator(".cm-content").waitFor({ timeout: 15000 });
    await p.waitForFunction(() => document.querySelector(".cm-content")?.textContent.includes("typed while cut off"), null, { timeout: 12000 });
    ok(true, "offline: a second reload still has it (the room kept it)");
    await ctx.close(); }
} catch (e) { console.log("FAIL  " + e.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
