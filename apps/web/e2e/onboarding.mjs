// The stepped onboarding on fixtures (?server=0&onboarding=1, a pretended passkey and a simulated machine): the owner's five steps and the invited person's three, each step confirming with a tick and moving on, a count-up for waits,
// skipping only where it is safe (and saying what it costs), resuming after a reload, keyboard-only operation, reduced motion, and no accessibility violation on any step.
import AxeBuilder from "@axe-core/playwright";
import { chromium } from "playwright";
import { BASE, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const heading = (p) => p.locator("h1").first().textContent();
const waitHeading = (p, t) => p.waitForFunction((t) => document.querySelector("main h1")?.textContent === t, t, { timeout: 20000 });
const axe = async (p, tag) => { const r = await new AxeBuilder({ page: p }).withTags(["wcag2a", "wcag2aa", "best-practice"]).analyze(); ok(r.violations.length === 0, `${tag}: no accessibility violation`, r.violations.map((v) => `${v.id}: ${v.nodes[0].target.join(" ")}`).join("; ")); };
const open = async (query, reduced = false) => { const ctx = await b.newContext({ viewport: { width: 1280, height: 860 }, reducedMotion: reduced ? "reduce" : "no-preference" }); const page = await ctx.newPage(); await page.goto(`${BASE}/?server=0&${query}`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500); return { ctx, page }; };
{
  const { ctx, page } = await open("onboarding=1");
  ok((await page.locator("ol.steps li").count()) === 5, "the owner has five numbered steps");
  ok((await heading(page)) === "Who you are" && (await page.locator("footer").getByRole("button", { name: "Skip" }).count()) === 0, "step 1 is who you are, and it cannot be skipped");
  ok(await page.evaluate(() => document.activeElement?.tagName === "H1"), "the step's heading takes focus when it appears");
  await axe(page, "step 1");
  // keyboard only: type the name, Enter creates the passkey
  await page.getByLabel("Your name").focus(); await page.keyboard.type("Kay"); await page.keyboard.press("Enter");
  await waitHeading(page, "Choose a look");
  ok((await page.locator("ol.steps li.ok").count()) === 1 && (await page.locator("ol.steps li.now").textContent()).includes("Look"), "step 1 ticked and the look step has the card");
  await axe(page, "step 2");
  ok((await page.locator("footer").getByRole("button", { name: "Skip" }).count()) === 1 && /change the look any time/.test(await page.locator("footer small").textContent()), "the look can be skipped, and says what that means");
  await page.reload(); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500);
  ok((await heading(page)) === "Choose a look", "a reload resumes at the step you were on");
  await page.getByRole("radio").nth(1).click();
  await waitHeading(page, "Your machine");
  await axe(page, "step 3 before the command");
  await page.getByRole("button", { name: "Show the command" }).click();
  await page.locator("code").first().waitFor();
  ok(/calternal machine enroll/.test(await page.locator("code").first().textContent()), "the machine step prints the command to run");
  await page.waitForFunction(() => document.querySelector(".wait"), null, { timeout: 5000 });
  ok(/s$/.test((await page.locator(".wait .n").first().textContent()).trim()), "a wait shows a count-up in seconds (after 400 ms, no spinner)");
  await page.getByRole("button", { name: /Copy the command/ }).click(); await page.waitForTimeout(350);
  ok((await page.locator(".cb .tk.on").count()) === 1, "copying turns the button's icon into the tick");
  await page.getByText("dev-machine").waitFor({ timeout: 20000 });
  ok(true, "the step sees the machine connect by itself");
  await axe(page, "step 3 connected");
  await page.getByRole("button", { name: "Continue" }).click(); await waitHeading(page, "Sign in to a provider");
  ok(/Not signed in/.test(await page.locator("li").first().textContent()), "the provider shows not signed in, with the sign-in command to copy");
  await page.getByRole("button", { name: "Check again" }).click(); await page.waitForTimeout(900);
  await page.getByRole("button", { name: "Check again" }).click();
  await page.getByText("Signed in", { exact: false }).first().waitFor({ timeout: 15000 });
  await axe(page, "step 4");
  await page.getByRole("button", { name: "Continue" }).click(); await waitHeading(page, "You are set up");
  ok((await page.locator("ol.steps li.ok").count()) === 4, "the four finished steps are ticks", `${await page.locator("ol.steps li.ok").count()}`);
  await axe(page, "done");
  ok((await page.getByRole("button", { name: "Start the first thread" }).count()) === 1, "the last step leads into the first thread");
  await ctx.close();
}
{
  const { ctx, page } = await open("onboarding=invited");
  ok((await page.locator("ol.steps li").count()) === 3, "an invited person has the shorter path: you, look, done");
  await page.getByLabel("Your name").fill("Sam"); await page.getByRole("button", { name: "Create passkey" }).click(); await waitHeading(page, "Choose a look");
  await page.getByRole("button", { name: "Skip" }).click(); await waitHeading(page, "You are set up");
  ok((await page.locator("ol.steps li").nth(1).textContent()).includes("skipped"), "a skipped step is marked skipped, not ticked");
  await ctx.close();
}
{
  const { ctx, page } = await open("onboarding=1", true);
  const anim = await page.evaluate(() => getComputedStyle(document.querySelector("section.in")).animationName);
  ok(/fade/.test(anim) || anim === "none", "reduced motion: the step enters with a fade, no slide", anim);
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);
