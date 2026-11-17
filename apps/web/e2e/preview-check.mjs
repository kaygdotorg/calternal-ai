// Checks a deployed copy (CAL_URL=http://127.0.0.1/ai node e2e/preview-check.mjs): loads, fonts in, deep links, in-app navigation under the base path.
import { BASE, launch, open } from "./lib.mjs";
const b = await launch();
let bad = 0;
const check = (name, ok, extra = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name} ${extra}`); if (!ok) bad++; };
try {
  const { page, ctx } = await open(b, "desktop", "light", "/threads/t-run");
  const info = await page.evaluate(async () => ({
    title: document.querySelector("h1")?.textContent,
    fonts: [...document.fonts].filter((f) => f.status === "loaded").map((f) => f.family.replace(/"/g, "")),
    body: getComputedStyle(document.body).fontFamily,
    path: location.pathname,
  }));
  check("deep link renders the thread", info.title === "Split store.rs by concern", info.title);
  check("Google Sans Flex loaded", info.fonts.includes("Google Sans Flex"), info.fonts.join(","));
  check("Maple Mono loaded", info.fonts.includes("Maple Mono"));
  const link = page.getByRole("link", { name: /Expired invite test/ });
  const href = await link.getAttribute("href");
  check("links carry the base path", href.startsWith(new URL(BASE).pathname.replace(/\/$/, "") + "/threads/"), href);
  await link.click();
  await page.waitForTimeout(400);
  check("in-app navigation, no reload", (await page.evaluate(() => location.pathname)).endsWith("/threads/t-idle"));
  await page.reload();
  await page.waitForSelector("html[data-ready]");
  check("reload of a deep link", (await page.locator("h1").first().textContent()) === "Expired invite test");
  const bad404 = [];
  page.on("response", (r) => r.status() >= 400 && bad404.push(r.url()));
  await page.goto(BASE + "/notes/n-1");
  await page.waitForSelector("html[data-ready]");
  check("no failed requests", bad404.length === 0, bad404.join(" "));
  await ctx.close();
} finally { await b.close(); }
process.exit(bad ? 1 : 0);
