// The screens that show when something is missing or wrong, in plain words with the next step (fixture scenarios, `?scenario=`): an empty account, an empty thread, no machine, a machine that is
// offline, a provider not signed in, a failed turn, a waiting retry, an interrupted turn. Screenshots go to /tmp/state-<name>-<device>.png.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const NOTICES = [["no-machine", /No machine to run this on yet/, "Open Machines"], ["offline", /is offline/, "Open Machines"], ["signin", /is not signed in on/, "Copy the command"], ["failed", /The last turn failed/, "Try again"], ["retrying", /Trying again in/, "Try now"], ["interrupted", /stopped answering/, "Resume"]];
for (const device of ["desktop", "phone"]) {
  for (const [name, title, action] of NOTICES) {
    const { page, ctx } = await open(b, device, "light", `/threads/t-idle?scenario=${name}`);
    const n = page.getByRole("status").filter({ hasText: title }).first();
    ok(`${device} ${name}: the notice shows`, (await n.count()) === 1);
    ok(`${device} ${name}: it offers "${action}"`, (await n.getByRole("button", { name: action }).count()) === 1);
    await page.screenshot({ path: `/tmp/state-${name}-${device}.png` });
    if (device === "desktop" && name === "no-machine") { await n.getByRole("button", { name: action }).click(); await page.waitForTimeout(500); ok("Open Machines goes to Settings > Machines", /\/settings\/machines/.test(page.url()), page.url()); }
    if (name === "failed") { await n.getByRole("button", { name: "Try again" }).click(); await page.waitForTimeout(400); ok(`${device}: Try again starts the turn and the notice goes`, (await page.getByRole("status").filter({ hasText: title }).count()) === 0); }
    await ctx.close();
  }
  // A fresh account: no threads
  const { page, ctx } = await open(b, device, "light", "/threads?scenario=empty");
  await page.waitForTimeout(500);
  const heads = page.getByText("No threads yet"), starts = page.getByRole("button", { name: "Start a thread" });
  // On a desktop the sidebar and the middle card both say it; on a phone the middle card does (the list is not showing).
  ok(`${device} empty: it says what a thread is and offers one action`, (await heads.count()) >= 1 && (await starts.count()) >= 1, `${await heads.count()} headings, ${await starts.count()} buttons`);
  await page.screenshot({ path: `/tmp/state-empty-${device}.png` });
  await starts.last().click(); await page.waitForTimeout(800);
  ok(`${device} empty: Start a thread opens a new thread`, /\/threads\/t-new-/.test(page.url()), page.url());
  ok(`${device} empty thread: says nothing is sent yet and how to start`, (await page.getByText("Nothing sent yet").count()) === 1 && (await page.getByRole("button", { name: "Write a message" }).count()) === 1);
  await page.screenshot({ path: `/tmp/state-blank-${device}.png` });
  await page.getByRole("button", { name: "Write a message" }).click();
  ok(`${device} empty thread: Write a message focuses the box`, await page.evaluate(() => document.activeElement?.id === "composer-field"));
  await ctx.close();
}
// The three request kinds, with their next steps
{
  const { page, ctx } = await open(b, "desktop", "light", "/threads/t-wait");
  const bar = page.getByRole("region", { name: "Waiting on you" });
  ok("the request bar shows the question with what happens next", (await bar.getByText("The agent is waiting for your answer").count()) === 1);
  await page.screenshot({ path: "/tmp/state-request-question.png" });
  await bar.getByRole("button", { name: "Next request" }).click(); await page.waitForTimeout(300);
  ok("the approval says nothing runs until you decide", (await bar.getByText(/Nothing runs until you decide/).count()) === 1);
  await page.screenshot({ path: "/tmp/state-request-approval.png" });
  await ctx.close();
  const s = await open(b, "desktop", "light", "/threads/t-secret");
  ok("the secret says where the value goes", (await s.page.getByText(/goes straight to Calternal/).count()) === 1);
  await s.page.screenshot({ path: "/tmp/state-request-secret.png" });
  await s.ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);
