// The account steps of the onboarding against fixtures (the server is pretended: the email code is 00000000): name and username with the server's rules, the email screen (optional, or required with ?email_required=1) and its
// eight-digit code, the passkey, then the recovery codes shown once (ten, in groups of eight, only an explicit "I have saved them" goes on) and the dismissible second passkey. Desktop and phone width.
// Usage: node e2e/with-preview.mjs e2e/onboarding-account.mjs
import { BASE, launch, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const cur = (p) => p.locator("ol.steps li[aria-current=step]").innerText().catch(() => "");
const btn = (p, n) => p.getByRole("button", { name: n });
try {
  for (const [device, vp] of [["desktop", { width: 1200, height: 900 }], ["phone", { width: 390, height: 844 }]]) {
    const ctx = await browser.newContext({ viewport: vp, hasTouch: device === "phone" }); const p = await ctx.newPage();
    await p.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    await p.goto(BASE + "/?onboarding=1"); await p.waitForSelector("html[data-ready]"); await p.locator("main ol.steps").waitFor();
    ok(/You/.test(await cur(p)), `${device}: the first step is You`);
    ok(await btn(p, "Continue").isDisabled(), `${device}: Continue waits for a name and a valid username`);
    await p.getByLabel("Your name").fill("Kay"); await p.getByLabel("Username").fill("ab");
    ok(/at least 3/.test(await p.locator("form").innerText()) && await btn(p, "Continue").isDisabled(), `${device}: a short username says so in plain words`);
    await p.getByLabel("Username").fill("admin");
    ok(/kept by the server/.test(await p.locator("form").innerText()), `${device}: a reserved username is refused`);
    await p.getByLabel("Username").fill("kay-g"); await btn(p, "Continue").click();
    ok(/optional/i.test(await p.locator("form").innerText()) && /administrator/.test(await p.locator("form").innerText()), `${device}: the email is optional here and says plainly that recovery then goes through the admin`);
    await btn(p, "Continue").click();
    ok(await btn(p, "Create passkey").isVisible(), `${device}: with no email the passkey screen follows`);
    await btn(p, "Create passkey").click(); await p.waitForTimeout(1900);
    ok(/Recovery/.test(await cur(p)), `${device}: the account is made and the recovery codes step follows`);
    const codes = await p.locator(".codes li code").allInnerTexts();
    ok(codes.length === 10 && codes.every((c) => /^[0-9a-f]{8}( [0-9a-f]{8}){3}$/.test(c)), `${device}: ten codes, four groups of eight`, String(codes.length));
    ok((await btn(p, "Continue").count()) === 0 && await btn(p, "I have saved them").isVisible(), `${device}: only an explicit "I have saved them" goes on`);
    const over = await p.evaluate(() => document.documentElement.scrollWidth - innerWidth);
    ok(over <= 0, `${device}: nothing is wider than the screen`, String(over));
    await btn(p, "I have saved them").click(); await p.waitForTimeout(1200);
    ok(/Second passkey/.test(await cur(p)), `${device}: then the second passkey`);
    await btn(p, "Skip").click(); await p.waitForTimeout(300);
    ok(/Look/.test(await cur(p)), `${device}: Skip is explicit and goes on`);
    await ctx.close();
  }
  // required email, the code typed back
  { const ctx = await browser.newContext({ viewport: { width: 1200, height: 900 } }); const p = await ctx.newPage();
    await p.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    await p.goto(BASE + "/?onboarding=1&email_required=1"); await p.waitForSelector("html[data-ready]"); await p.locator("main ol.steps").waitFor();
    await p.getByLabel("Your name").fill("Kay"); await p.getByLabel("Username").fill("kay-g"); await btn(p, "Continue").click();
    ok(!/optional/i.test(await p.locator("form").innerText()) && await btn(p, "Continue").isDisabled(), "hosted: the email is required and nothing goes on without it");
    await p.getByLabel("Email").fill("kay@example.com"); await btn(p, "Send the code").click(); await p.waitForTimeout(900);
    ok(/eight-digit code/.test(await p.locator("form").innerText()), "hosted: an eight-digit code is asked for (typed back, not a link)");
    await p.getByLabel("Code").fill("12345678"); await btn(p, "Confirm").click(); await p.waitForTimeout(700);
    ok(/not right/.test(await p.locator("form").innerText()), "hosted: a wrong code says so");
    ok(await btn(p, "Send it again").isVisible() && await btn(p, "Use a different address").isVisible(), "hosted: resend and a different address are offered");
    await p.getByLabel("Code").fill("0000 0000"); await btn(p, "Confirm").click(); await p.waitForTimeout(700);
    ok(await btn(p, "Create passkey").isVisible(), "hosted: the right code goes on to the passkey");
    await ctx.close(); }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
