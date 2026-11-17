// The phone screen set at 390 wide with the iPhone's safe areas (62 top, 34 bottom) applied, for the pass against docs/native-ui-study.md section 13.
// Usage: node e2e/phone-shots.mjs <outdir>
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { open, serve } from "./lib.mjs";
const out = process.argv[2] ?? "/tmp/phone"; mkdirSync(out, { recursive: true });
const stop = await serve(); const b = await chromium.launch();
const SAFE = ":root { --safe-top: 62px !important; --safe-bottom: 34px !important; }";
const shot = async (name, scheme, path, act) => {
  const { ctx, page } = await open(b, "phone", scheme, path);
  await page.addStyleTag({ content: SAFE });
  if (act) await act(page);
  await page.waitForTimeout(600);
  await page.screenshot({ path: `${out}/phone-${name}.png` });
  await ctx.close();
  console.log("shot", name);
};
const tap = (p, name) => p.getByRole("button", { name }).click();
await shot("thread", "light", "/threads/t-run");
await shot("thread-dark", "dark", "/threads/t-run");
await shot("sidebar", "light", "/threads/t-run", (p) => tap(p, "Show threads"));
await shot("right-panel", "light", "/threads/t-run", (p) => p.getByRole("tab", { name: "Changes" }).click());
await shot("request", "light", "/threads/t-wait");
await shot("composer-picker", "light", "/threads/t-idle", async (p) => { await p.locator("#composer-field").click(); await p.locator("#composer-field").pressSequentially("ask @pr"); });
await shot("provider-menu", "light", "/threads/t-run", (p) => p.locator(".chip.prov button").click());
await shot("permission-menu", "light", "/threads/t-run", (p) => p.getByRole("button", { name: "Approve for me" }).click());
await shot("palette", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Show threads" }).click(); await p.waitForTimeout(400); await p.getByRole("button", { name: "Search", exact: true }).click(); });
await shot("context-menu", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Show threads" }).click(); await p.waitForTimeout(500); await p.locator("a.row").first().click({ button: "right" }); });
await shot("account", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Show threads" }).click(); await p.waitForTimeout(400); await p.getByRole("button", { name: /Account/ }).click(); });
await shot("settings-list", "light", "/settings", (p) => tap(p, /Show settings|Show threads|sidebar/i).catch(() => {}));
await shot("settings-page", "light", "/settings/agents");
await shot("notes", "light", "/notes/n-1");
await shot("appearance-sheet", "light", "/settings/appearance?sheet=1");
await shot("first-run", "light", "/threads?firstrun=1");
await b.close(); stop();
