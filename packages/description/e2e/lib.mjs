// Shared by the browser suites of this package: serves the built harness (`npm run harness:build`) and opens Chromium on it.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
export const PORT = Number(process.env.HARNESS_PORT ?? 4381);
export const BASE = `http://127.0.0.1:${PORT}`;

/** Starts `vite preview` of the harness build and resolves with a stop function. */
export async function serve() {
  if (!existsSync(join(here, "../dist-harness/index.html"))) { console.error("No dist-harness: run `npm run harness:build -w @calternal/description` first."); process.exit(2); }
  const p = spawn("npx", ["vite", "preview", "-c", "vite.harness.config.ts", "--port", String(PORT)], { cwd: join(here, ".."), stdio: "ignore", detached: true });
  for (let i = 0; i < 80; i++) { try { if ((await fetch(BASE)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
  return () => { try { process.kill(-p.pid); } catch { p.kill(); } };
}

/** Opens the harness with a query string; returns the page and its CDP session. */
export async function open(browser, query, { width = 1280, height = 800 } = {}) {
  const ctx = await browser.newContext({ viewport: { width, height } });
  const page = await ctx.newPage();
  const cdp = await ctx.newCDPSession(page);
  await page.goto(`${BASE}/?${query}`);
  return { page, ctx, cdp };
}

/** `CAL_CHROME` names a Chromium binary when the one Playwright expects is not installed (dev-02 has another lane's build). */
export const launch = () => chromium.launch(process.env.CAL_CHROME ? { executablePath: process.env.CAL_CHROME } : {});
export const pct = (xs, q) => { const s = [...xs].sort((a, b) => a - b); return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * q))] : NaN; };
