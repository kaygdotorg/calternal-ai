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
    await p.getByRole("button", { name: /Here now: Maya Okafor/ }).waitFor({ timeout: 8000 });
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
  // Access taken away while writing: the membership ends, the edits are kept as a note in the personal space, and the person is told in one line.
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/notes/n-2?firstrun=0&live=1&peer=0&test=1");
    await p.locator(".cm-content").waitFor({ timeout: 15000 });
    await p.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
    await p.locator(".cm-content").click(); await p.keyboard.press("Control+End"); await p.keyboard.type("\n\nWritten as access ended.");
    await p.evaluate(() => window.__net.revoke());
    await p.getByText(/Your edits were kept as “Your edits to Storage layout” in your personal space/).waitFor({ timeout: 8000 });
    ok(true, "revoked: the person is told in one line where their edits went");
    await p.keyboard.type(" more");
    ok(!(await p.locator(".cm-content").innerText()).includes(" more"), "revoked: nothing more can be typed into the note");
    await ctx.close(); }
  // Names beside cursors on a touch screen: a name shows when somebody's cursor moves and goes again; pressing the avatars shows all until the next press elsewhere.
  { const { page: p, ctx } = await open(browser, "phone", "dark", "/notes/n-1?firstrun=0&live=1");
    await p.locator(".cm-ySelectionCaret").first().waitFor({ timeout: 8000 });
    await p.locator(".cm-editor[data-labels=move]").waitFor({ timeout: 8000 });
    ok(true, "labels: a name shows when another person's cursor moves");
    await p.waitForFunction(() => !document.querySelector(".cm-editor")?.hasAttribute("data-labels"), null, { polling: 40, timeout: 6000 });
    ok(true, "labels: and goes again after about two seconds");
    await p.getByRole("button", { name: /Here now: Maya Okafor/ }).tap();
    await p.locator(".cm-editor[data-labels=all]").waitFor({ timeout: 2000 });
    await p.waitForTimeout(2600);
    ok(await p.locator(".cm-editor[data-labels=all]").count() === 1, "labels: pressing the avatars keeps every name shown");
    await p.locator(".cm-content").tap({ position: { x: 20, y: 20 } });
    await p.waitForTimeout(300);
    ok(await p.locator(".cm-editor[data-labels=all]").count() === 0, "labels: the next press elsewhere hides them");
    await ctx.close(); }
  // Edits owed after access ended survive a closed tab and an unreachable server: kept on the device, sent once when it is back.
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/notes/n-2?firstrun=0&live=1&peer=0&test=1");
    await p.evaluate(() => { localStorage.setItem("cal.fixture.rescue-down", "1"); localStorage.removeItem("cal.fixture.rescued"); });
    await p.locator(".cm-content").waitFor({ timeout: 15000 });
    await p.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
    await p.locator(".cm-content").click(); await p.keyboard.press("Control+End"); await p.keyboard.type("\n\nOwed while offline.");
    await p.evaluate(() => window.__net.revoke());
    await p.getByText(/kept on this device and go to your personal space/).waitFor({ timeout: 8000 });
    ok(true, "owed: with the server away the line says the edits are kept on this device");
    await p.reload(); await p.locator(".cm-content").waitFor({ timeout: 15000 }); await p.waitForTimeout(800);
    ok(await p.evaluate(() => (JSON.parse(localStorage.getItem("cal.fixture.rescued") ?? "[]")).length) === 0, "owed: still unsent after a reload while the server is away");
    await p.evaluate(() => { localStorage.removeItem("cal.fixture.rescue-down"); dispatchEvent(new Event("online")); });
    await p.getByText(/Your edits were kept as “Your edits to Storage layout” in your personal space/).waitFor({ timeout: 8000 });
    ok(true, "owed: when it is back the line says where the edits went");
    await p.reload(); await p.locator(".cm-content").waitFor({ timeout: 15000 }); await p.waitForTimeout(1000);
    ok(await p.evaluate(() => (JSON.parse(localStorage.getItem("cal.fixture.rescued") ?? "[]")).length) === 1, "owed: the rescued note exists once, however often it was retried");
    await ctx.close(); }
  // Rescue holds only what this person wrote: an unconfirmed local change, then a peer's later change, then access ends.
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/notes/n-2?firstrun=0&live=1&test=1");
    await p.evaluate(() => localStorage.removeItem("cal.fixture.rescued"));
    await p.locator(".cm-content").waitFor({ timeout: 15000 });
    await p.getByText("Saved", { exact: true }).waitFor({ timeout: 8000 });
    await p.evaluate(() => window.__net.cut()); // the local change stays unconfirmed
    await p.locator(".cm-content").click(); await p.keyboard.press("Control+End"); await p.keyboard.type("\n\nMine, unconfirmed.");
    await p.evaluate(() => window.__net.restore()); await p.evaluate(() => window.__peer.type("\n\nPeer wrote this meanwhile."));
    await p.getByText("Peer wrote this meanwhile.").waitFor({ timeout: 8000 });
    await p.evaluate(() => window.__net.revoke());
    await p.getByText(/Your edits were kept as/).waitFor({ timeout: 8000 });
    const blocks = await p.evaluate(() => JSON.parse(localStorage.getItem("cal.fixture.rescued") ?? "[]")[0]?.blocks ?? []);
    ok(blocks.join(" ").includes("Mine, unconfirmed.") && !blocks.join(" ").includes("Peer wrote this"), "rescue: only this person's own words are kept, never a peer's later change", JSON.stringify(blocks));
    await ctx.close(); }
} catch (e) { console.log("FAIL  " + e.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
