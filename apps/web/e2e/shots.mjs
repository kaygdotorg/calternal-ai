// Screenshots for docs/ui-shell-status.md. Usage: node e2e/shots.mjs <outdir>   (after `npm run build`).
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";

const out = process.argv[2] ?? "e2e/out";
mkdirSync(out, { recursive: true });
const stop = await serve();
const browser = await launch();
const shot = async (name, device, scheme, path, act, clip) => {
  const { ctx, page } = await open(browser, device, scheme, path);
  if (act) await act(page);
  await page.screenshot({ path: `${out}/${name}.jpg`, type: "jpeg", quality: 92, ...(clip ? { clip } : {}) });
  await ctx.close();
  console.log("shot", name);
};
try {
  await shot("desktop-light-stream", "desktop", "light", "/threads/t-run?panel=changes");
  await shot("desktop-dark-stream", "desktop", "dark", "/threads/t-run?panel=changes");
  await shot("desktop-light-request-question", "desktop", "light", "/threads/t-wait?panel=run");
  await shot("desktop-light-request-approval", "desktop", "light", "/threads/t-wait?panel=run", async (p) => { await p.getByRole("button", { name: "Next request" }).click(); await p.waitForTimeout(300); });
  await shot("desktop-dark-request-secret", "desktop", "dark", "/threads/t-secret?panel=files");
  await shot("desktop-light-long-thread", "desktop", "light", "/threads/t-long?panel=run");
  await shot("desktop-light-notes", "desktop", "light", "/notes/n-1?panel=inspector", async (p) => { await p.getByRole("button", { name: "Table of contents" }).focus(); await p.waitForTimeout(300); });
  await shot("desktop-light-switcher", "desktop", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Threads", exact: true }).last().click(); await p.waitForTimeout(300); });
  await shot("desktop-light-permission-menu", "desktop", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Approve for me" }).click(); await p.waitForTimeout(300); });
  await shot("phone-light-permission-menu", "phone", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "dev-02-std · Sandboxed" }).click(); await p.waitForTimeout(300); });
  await shot("desktop-light-brand-crop", "desktop", "light", "/threads/t-run", async () => {}, { x: 0, y: 0, width: 340, height: 72 });
  await shot("desktop-light-settings", "desktop", "light", "/settings/appearance");
  await shot("desktop-dark-settings-fonts", "desktop", "dark", "/settings/appearance", async (p) => { await p.getByRole("button", { name: /Interface/ }).click(); await p.waitForTimeout(400); });
  await shot("desktop-dark-wallpaper-dusk", "desktop", "dark", "/threads/t-run", async (p) => { await p.evaluate(() => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: "dusk" }))); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200); });
  await shot("desktop-light-wallpaper-dune", "desktop", "light", "/threads/t-run", async (p) => { await p.evaluate(() => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: "dune" }))); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200); });
  await shot("phone-light-settings", "phone", "light", "/settings/appearance");
  await shot("phone-dark-settings", "phone", "dark", "/settings/appearance");
  await shot("phone-light-thread", "phone", "light", "/threads/t-run");
  await shot("phone-dark-thread", "phone", "dark", "/threads/t-run");
  await shot("phone-light-request", "phone", "light", "/threads/t-wait");
  await shot("phone-light-sidebar", "phone", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Show threads" }).click(); await p.waitForTimeout(500); });
  await shot("phone-light-right-card", "phone", "light", "/threads/t-run", async (p) => { await p.getByRole("tab", { name: "Changes" }).click(); await p.waitForTimeout(500); });
} finally {
  await browser.close();
  stop();
}
