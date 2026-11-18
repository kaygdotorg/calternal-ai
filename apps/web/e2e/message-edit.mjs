// Editing a sent message as a fork inside the same thread (docs/threads-ui.md, "Editing a sent message"): the bubble becomes an editor, the edit is another version (the old tail kept), one version shows at a time, the switch steps through them,
// Escape cancels, and no second thread appears. Usage: CAL_URL=... node e2e/message-edit.mjs
import { launch, open, serve, allThreads } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  const { page: p, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
  await allThreads(p); await p.waitForTimeout(600);
  const rowsBefore = await p.locator("aside.side a.row").count();
  const users = p.locator(".msg.user");
  const mine = users.last();
  await mine.scrollIntoViewIfNeeded(); await mine.hover(); await p.waitForTimeout(400);
  const original = (await mine.locator(".bubble").innerText()).trim();
  const agentsAfter = async () => p.locator(".msg.agent-row").count();
  // Escape cancels
  await mine.getByRole("button", { name: "Edit message" }).click();
  ok((await p.locator("[aria-label='Edit your message']").count()) === 1, "Edit turns the bubble into an editor in place");
  ok((await p.locator("[aria-label='Edit your message']").inputValue()) === original, "the editor holds the message's words");
  await p.keyboard.press("Escape"); await p.waitForTimeout(200);
  ok((await p.locator("[aria-label='Edit your message']").count()) === 0 && (await mine.locator(".bubble").innerText()).trim() === original, "Escape cancels: nothing changed");
  // send an edit
  await mine.hover(); await mine.getByRole("button", { name: "Edit message" }).click();
  await p.keyboard.press("Control+a"); await p.keyboard.type("An edited question about the journal"); await p.keyboard.press("Enter");
  await p.waitForTimeout(900);
  const last = p.locator(".msg.user").last();
  ok((await last.innerText()).includes("An edited question about the journal"), "Enter sends: the message now shows the edit");
  ok(/2 \/ 2/.test(await last.innerText()) && /Edited/.test(await last.innerText()), "it shows version 2 of 2 and Edited");
  // the agent answers on the new branch (a running turn), and the old tail is not on screen
  await p.waitForFunction(() => document.querySelectorAll(".msg.agent-row").length >= 1 && !document.querySelector(".working"), null, { timeout: 15000 }).catch(() => {});
  // go back to the original
  await last.hover(); await last.getByRole("button", { name: "Previous version" }).click(); await p.waitForTimeout(700);
  const back = p.locator(".msg.user", { hasText: "1 / 2" }).last();
  ok((await back.count()) === 1 && (await back.locator(".bubble").innerText()).trim() === original, "Previous version shows the original message again", (await back.locator(".bubble").innerText().catch(() => "")).slice(0, 40));
  ok(!/Edited/.test(await back.innerText()), "the first version is not marked Edited");
  ok((await agentsAfter()) > 0, "the old tail is back (nothing was deleted)");
  // keys
  await back.getByRole("button", { name: "Next version" }).focus(); await p.keyboard.press("Alt+BracketRight"); await p.waitForTimeout(600);
  ok(/2 \/ 2/.test(await p.locator(".msg.user", { hasText: "2 / 2" }).last().innerText().catch(() => "")), "Alt+] shows the next version");
  await p.locator(".msg.user", { hasText: "2 / 2" }).last().getByRole("button", { name: "Previous version" }).focus(); await p.keyboard.press("Alt+BracketLeft"); await p.waitForTimeout(600);
  ok((await p.locator(".msg.user", { hasText: "1 / 2" }).count()) === 1, "Alt+[ shows the previous version again");
  ok((await p.locator("aside.side a.row").count()) === rowsBefore, "no second thread appears in the list", `${rowsBefore} rows`);
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
