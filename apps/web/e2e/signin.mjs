// The provider sign-in flow (docs/threads-ui.md, "Provider sign-in"): a request in the bar and an item in the conversation with the link, the code and a field for the pasted code; "Authentication done" when it ends;
// the inbox lists it; the typing hook is called once per focus. Fixtures: ?scenario=signin-request (needs a pasted code) and signin-link (ends by itself). Usage: CAL_URL=... node e2e/signin.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  {
    const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-idle?scenario=signin-request");
    await page.waitForTimeout(800);
    const bar = page.getByRole("region", { name: "Waiting on you" });
    ok((await bar.count()) === 1 && /Claude needs you to sign in again/.test(await bar.innerText()), "the bar says \"Claude needs you to sign in again\"");
    const item = page.locator(".signin").first();
    ok(/Claude needs you to sign in again/.test(await item.innerText()) && /example\.com\/sign-in/.test(await item.innerText()) && /WXYZ-1234/.test(await item.innerText()), "the conversation has the same item with the link and the code");
    ok((await item.locator("a[href='https://example.com/sign-in']").getAttribute("target")) === "_blank", "the link opens in a new tab");
    await page.locator("aside.side [role=tab]", { hasText: "Inbox" }).click();
    ok(/Sign in/.test(await page.locator("aside.side [role=tabpanel]").innerText()), "the Inbox lists the thread as Sign in");
    const field = item.getByRole("textbox", { name: "Code from the provider" });
    ok((await item.getByRole("button", { name: "Continue" }).isDisabled()), "Continue waits for a code");
    await field.fill("abc-123"); await item.getByRole("button", { name: "Continue" }).click();
    await page.waitForTimeout(800);
    ok(/Authentication done/.test(await page.locator(".signin").first().innerText()), "answering turns the item into \"Authentication done\"");
    ok((await page.getByRole("region", { name: "Waiting on you" }).count()) === 0, "the bar is gone");
    await page.waitForTimeout(9000);
    ok((await page.locator(".signin").count()) === 0, "nothing of it stays after a few seconds");
    await ctx.close();
  }
  {
    const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
    const n = () => page.evaluate(() => window.__typing ?? 0);
    await page.locator("#composer-field").click(); await page.keyboard.type("hello"); await page.waitForTimeout(200);
    ok((await n()) === 1, "typing in a thread asks the server about a sign-in once per focus", String(await n()));
    await page.keyboard.type(" more"); ok((await n()) === 1, "more typing in the same focus does not ask again");
    await page.locator("aside.side").click({ position: { x: 5, y: 5 } }); await page.waitForTimeout(300);
    await page.locator("#composer-field").click(); await page.keyboard.type("x"); await page.waitForTimeout(200);
    ok((await n()) === 2, "a new focus asks again", String(await n()));
    await ctx.close();
  }
  {
    const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-idle?scenario=signin-link");
    await page.waitForTimeout(500);
    ok((await page.locator(".signin input").count()) === 0, "a link-only sign-in has no field for a code");
    await page.waitForFunction(() => /Authentication done/.test(document.querySelector(".signin")?.textContent ?? ""), null, { timeout: 15000 }).then(() => ok(true, "a link-only sign-in turns into \"Authentication done\" by itself when the status says done"), () => ok(false, "a link-only sign-in turns into \"Authentication done\" by itself"));
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
