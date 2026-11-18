// Message actions in fixtures (the server's own run is in steps-server.mjs): see actions-check.mjs. Usage: CAL_URL=... node e2e/message-actions.mjs
import { DEVICES, serve, BASE } from "./lib.mjs";
import { chromium } from "playwright";
import { checkMessageActions } from "./actions-check.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (const device of ["desktop", "phone"]) {
    console.log(`-- ${device}`);
    const ctx = await b.newContext({ ...DEVICES[device], colorScheme: "dark" });
    await ctx.grantPermissions(["clipboard-read", "clipboard-write"], { origin: new URL(BASE).origin }).catch(() => {});
    await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    const page = await ctx.newPage(); await page.goto(BASE + "/threads/t-idle"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
    await checkMessageActions(page, ok, device);
    await ctx.close();
  }
} finally { await b.close(); stop(); }
process.exit(bad ? 1 : 0);
