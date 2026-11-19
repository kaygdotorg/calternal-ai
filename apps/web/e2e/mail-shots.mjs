// The Mail screens on the fixture mailbox, for looking at: the list with a message open, the account form and the empty state, on a desktop and a 393 pt phone, in light and dark, with Notes alongside for comparison.
// Usage: node e2e/mail-shots.mjs <outdir>   (after `npm run build -w apps/web`)
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { open, serve } from "./lib.mjs";
const out = process.argv[2] ?? "/tmp/mail-shots"; mkdirSync(out, { recursive: true });
const stop = await serve(); const b = await chromium.launch();
const SAFE = ":root { --safe-top: 62px !important; --safe-bottom: 34px !important; }";
const shot = async (name, device, scheme, path, act) => {
  const { ctx, page } = await open(b, device, scheme, path);
  if (device === "phone") { await page.setViewportSize({ width: 393, height: 852 }); await page.addStyleTag({ content: SAFE }); }
  if (act) await act(page);
  await page.waitForTimeout(700);
  await page.screenshot({ path: `${out}/${device}-${name}-${scheme}.png` });
  await ctx.close(); console.log("shot", device, name, scheme);
};
for (const scheme of ["light", "dark"]) {
  await shot("message", "desktop", scheme, "/mail/200", (p) => p.getByRole("heading", { level: 1 }).waitFor());
  await shot("attachments", "desktop", scheme, "/mail/199", (p) => p.getByText("receipt-4411.pdf").waitFor());
  await shot("setup", "desktop", scheme, "/mail/setup");
  await shot("notes", "desktop", scheme, "/notes");
  await shot("list", "phone", scheme, "/mail", (p) => p.getByRole("button", { name: "Show the sidebar" }).click());
  await shot("message", "phone", scheme, "/mail/199", (p) => p.getByText("receipt-4411.pdf").waitFor());
  await shot("setup", "phone", scheme, "/mail/setup");
  await shot("notes-list", "phone", scheme, "/notes", (p) => p.getByRole("button", { name: /Show the sidebar/ }).click());
}
await b.close(); stop();
