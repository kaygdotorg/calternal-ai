import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
const { page: p } = await open(browser, "desktop", "light", "/threads/t-run");
const t0 = Date.now(); const out = [];
for (let i = 0; i < 24; i++) {
  await p.waitForTimeout(100);
  out.push(await p.evaluate(async () => { await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); const f = document.querySelector("[role=feed]"); const msgs = [...f.querySelectorAll(".msg")]; const last = msgs.at(-1).getBoundingClientRect(); const b = document.querySelector(".thread .bottom").getBoundingClientRect(); const c = document.querySelector(".composer").getBoundingClientRect(); return [Math.round(last.bottom), Math.round(b.top), Math.round(b.height), Math.round(c.height), parseInt(getComputedStyle(f).getPropertyValue("--fade-b")), Math.round(f.scrollHeight - f.scrollTop - f.clientHeight), msgs.length]; }));
}
console.log(out.map((x) => x.join(",")).join("\n"));
await browser.close(); await stop();
