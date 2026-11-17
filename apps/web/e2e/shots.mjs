// Screenshots for docs/ui-shell-status.md. Usage: node e2e/shots.mjs <outdir>   (after `npm run build`).
import { mkdirSync } from "node:fs";
import { launch, open, serve } from "./lib.mjs";

const out = process.argv[2] ?? "e2e/out";
mkdirSync(out, { recursive: true });
const stop = await serve();
const browser = await launch();
const shot = async (name, device, scheme, path, act) => {
  const { ctx, page } = await open(browser, device, scheme, path);
  if (act) await act(page);
  await page.screenshot({ path: `${out}/${name}.png` });
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
  await shot("phone-light-thread", "phone", "light", "/threads/t-run");
  await shot("phone-dark-thread", "phone", "dark", "/threads/t-run");
  await shot("phone-light-request", "phone", "light", "/threads/t-wait");
  await shot("phone-light-sidebar", "phone", "light", "/threads/t-run", async (p) => { await p.getByRole("button", { name: "Show threads" }).click(); await p.waitForTimeout(500); });
  await shot("phone-light-right-card", "phone", "light", "/threads/t-run", async (p) => { await p.getByRole("tab", { name: "Changes" }).click(); await p.waitForTimeout(500); });
} finally {
  await browser.close();
  stop();
}
