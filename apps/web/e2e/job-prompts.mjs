// Standalone plugin job prompts (a durable wait's question or approval, docs/plugin-api.md "Durable job waits"): they are rows of the Inbox on their own, with the reason mark, counted in the tab, and answered in a sheet; answering sends the answer and the row leaves.
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (const device of ["desktop", "phone"]) {
    const { page: p, ctx } = await open(browser, device, "dark", "/threads/t-idle");
    await p.waitForTimeout(800);
    if (device === "phone") { await p.getByRole("button", { name: "Show threads" }).click(); await p.waitForTimeout(900); }
    const rows = p.locator("[role=tabpanel] ul[aria-label='Waiting for you'] li[data-job]");
    ok((await rows.count()) === 2, `${device}: two plugin prompts are rows of the Inbox`, String(await rows.count()));
    ok(/Question/i.test(await rows.nth(0).innerText().catch(() => "")) || (await rows.nth(0).locator("[aria-label*=Question], [title*=Question]").count()) >= 0, `${device}: the row carries its reason`);
    const tab = await p.locator("[role=tablist] [role=tab]").first().innerText().catch(() => "");
    ok(/5/.test(tab), `${device}: the Inbox tab counts them with the threads`, tab.replace(/\s+/g, " "));
    await rows.nth(0).getByRole("link").or(rows.nth(0).locator("button, [role=button], a")).first().click();
    await p.waitForTimeout(500);
    const sheet = p.getByRole("dialog", { name: /from a plugin/ });
    ok((await sheet.count()) === 1, `${device}: pressing the row opens the prompt in a sheet`);
    await sheet.getByText("Release notes").first().waitFor({ timeout: 8000 }).catch(() => {});
    ok(/Release notes\b.*announcements/s.test(await sheet.innerText()), `${device}: the sheet draws the plugin form`, (await sheet.innerText()).replace(/\s+/g, " ").slice(0, 160));
    await sheet.getByRole("button", { name: /Send|Answer|Submit/i }).first().click();
    await p.waitForTimeout(700);
    ok((await rows.count()) === 1 && (await p.getByRole("dialog", { name: /from a plugin/ }).count()) === 0, `${device}: answering closes the sheet and the row leaves`);
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
finally { await browser.close(); await stop(); }
process.exit(bad ? 1 : 0);
