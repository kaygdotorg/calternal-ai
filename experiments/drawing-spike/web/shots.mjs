// Captures the spike's web screens (fixture data only) at desktop and phone width, light and dark, into $SHOTS (default
// /tmp/spike-drawing-shots). Needs `npx vite preview --port 4391` running. Evidence goes on Forgejo #107, never in git.
import { chromium } from "playwright";
import { mkdirSync } from "node:fs";
const out = process.env.SHOTS ?? "/tmp/spike-drawing-shots", base = "http://127.0.0.1:4391/";
mkdirSync(out, { recursive: true });
const scenes = [
  ["rl-sidebar", "app=reading-list&open=Unread"],
  ["rl-list", "app=reading-list&open=All&view=list"],
  ["rl-board", "app=reading-list&open=All&view=board"],
  ["rl-add", "app=reading-list&open=Unread", async (p) => p.getByRole("button", { name: "Add link" }).first().click()],
  ["rl-menu", "app=reading-list&open=Unread", async (p) => p.locator("main [data-item=r1] button.main").click({ button: "right" })],
  ["chat-sidebar", "app=calternal-chat"],
  ["chat", "app=calternal-chat&item=c1"],
  ["chat-top", "app=calternal-chat&item=c1", async (p) => { for (let i = 0; i < 4; i++) { await p.locator("[role=log]").evaluate((e) => (e.scrollTop = 0)); await p.waitForTimeout(250); } }],
  ["chat-menu", "app=calternal-chat&item=c1", async (p) => p.locator("[data-rec=a22]").click({ button: "right" })],
  ["chat-reply", "app=calternal-chat&item=c1", async (p) => { await p.locator("[data-rec=a26]").click({ button: "right" }); await p.getByRole("menuitem", { name: "Reply" }).click(); await p.locator("textarea").fill("I will post it at noon."); }],
  ["chat-edit", "app=calternal-chat&item=c1", async (p) => { await p.locator("[data-rec=a22]").click({ button: "right" }); await p.getByRole("menuitem", { name: "Edit" }).click(); }],
];
const sizes = [["desktop", { width: 1280, height: 820 }, 1], ["phone", { width: 390, height: 844 }, 3]];
const browser = await chromium.launch();
for (const [size, viewport, scale] of sizes) for (const scheme of ["light", "dark"]) {
  const ctx = await browser.newContext({ viewport, deviceScaleFactor: scale, colorScheme: scheme, hasTouch: size === "phone" });
  for (const [name, query, act] of scenes) {
    if (size === "phone" && name === "rl-sidebar") { /* the phone shows the sidebar alone */ }
    const q = size === "phone" && (name === "rl-sidebar") ? "app=reading-list" : query;
    const p = await ctx.newPage();
    await p.goto(`${base}?${q}&scheme=${scheme}`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(400);
    if (act) { await act(p); await p.waitForTimeout(450); }
    await p.screenshot({ path: `${out}/web-${size}-${scheme}-${name}.png` }); await p.close();
  }
  await ctx.close();
}
await browser.close();
console.log(`shots in ${out}`);
