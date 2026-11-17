// The review set for the owner and coordinator (attached to issue #15, never committed). Usage: node e2e/review-shots.mjs <outdir>
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { open, serve } from "./lib.mjs";
const out = process.argv[2] ?? "/tmp/review";
mkdirSync(out, { recursive: true });
const stop = await serve();
const browser = await chromium.launch();
const shot = async (name, device, scheme, path, act) => {
  const { ctx, page } = await open(browser, device, scheme, path);
  if (act) await act(page);
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${out}/${name}.png` });
  await ctx.close();
  console.log("shot", name);
};
const sections = (p) => p.evaluate(() => localStorage.setItem("cal.threads.layout", JSON.stringify({ sections: [{ id: "s-a", name: "Storage", collapsed: false, threadIds: ["t-run", "t-wait"] }, { id: "s-b", name: "Later", collapsed: true, threadIds: [] }], loose: [] }))).then(() => p.reload()).then(() => p.waitForTimeout(400));
try {
  await shot("desktop-light", "desktop", "light", "/threads/t-run?panel=changes");
  await shot("desktop-dark", "desktop", "dark", "/threads/t-run?panel=changes");
  await shot("phone-light", "phone", "light", "/threads/t-run");
  await shot("phone-dark", "phone", "dark", "/threads/t-run");
  await shot("sidebar-sections", "desktop", "light", "/threads/t-run", sections);
  await shot("context-menu", "desktop", "light", "/threads/t-run", async (p) => { await sections(p); await p.locator("a.row").first().click({ button: "right" }); });
  await shot("model-picker", "desktop", "light", "/threads/t-run", async (p) => { await p.locator(".chip.prov button").click(); });
  await shot("toast-archive-undo", "desktop", "light", "/threads", async (p) => { await p.locator("a.row").first().click({ button: "right" }); await p.getByRole("menuitem", { name: "Archive" }).click(); await p.waitForTimeout(500); });
  await shot("request-bar", "desktop", "light", "/threads/t-wait?panel=run");
  await shot("shortcuts-sheet", "desktop", "light", "/threads/t-run", async (p) => { await p.keyboard.press("Control+/"); });
  await shot("selection-actions", "desktop", "light", "/threads/t-idle", async (p) => { const m = p.locator(".selectable").first(); await m.dblclick(); await p.waitForTimeout(300); });
  await shot("phone-machine-sheet", "phone", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: /Sandboxed/ }).click(); });
  await shot("phone-sidebar", "phone", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Show threads" }).click(); });
} finally { await browser.close(); stop(); }
