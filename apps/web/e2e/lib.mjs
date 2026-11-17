// Shared by the e2e scripts: starts `vite preview` of the built app, opens Chromium, and exposes small helpers.
import { spawn } from "node:child_process";
import { chromium } from "playwright";

export const PORT = 4174;
export const BASE = `http://127.0.0.1:${PORT}`;

/** Starts the preview server and resolves when it answers. Returns a stop function. */
export async function serve() {
  const p = spawn("npx", ["vite", "preview", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: false });
  for (let i = 0; i < 60; i++) {
    try { if ((await fetch(BASE)).ok) break; } catch { /* not yet */ }
    await new Promise((r) => setTimeout(r, 250));
  }
  return () => p.kill();
}

/** The devices the screenshots and checks use. */
export const DEVICES = {
  desktop: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 },
  phone: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
};

/** Opens a page on a device in a scheme and waits until the app has painted (fonts in). */
export async function open(browser, device, scheme, path) {
  const ctx = await browser.newContext({ ...DEVICES[device], colorScheme: scheme, reducedMotion: "no-preference" });
  const page = await ctx.newPage();
  await page.goto(BASE + path);
  await page.waitForSelector("html[data-ready]");
  await page.waitForTimeout(400);
  return { ctx, page };
}

export const launch = () => chromium.launch();
