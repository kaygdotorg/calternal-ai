import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
for (const dev of ["desktop", "phone"]) {
  const { page: p } = await open(browser, dev, "dark", "/threads/t-run");
  await p.waitForTimeout(1200);
  console.log(dev, JSON.stringify(await p.evaluate(() => [...document.querySelectorAll("[data-win]")].map((w) => [w.dataset.kind, w.dataset.item, w.hasAttribute("data-focused"), Math.round(w.getBoundingClientRect().left), Math.round(w.getBoundingClientRect().width)]))));
}
await browser.close(); await stop();
