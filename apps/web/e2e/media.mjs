// Pill-contrast hazards, pasted wallpaper links and the picture storage list.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
{ // hazards
  const { page, ctx } = await open(b, "desktop", "light", "/threads/t-run");
  await page.waitForSelector("h1");
  const pillBox = await page.locator("header .title").boundingBox();
  await page.evaluate(([x, y]) => { const feed = document.querySelector("[role=feed]"); const fr = feed.getBoundingClientRect(); x -= fr.left; y = y - fr.top + feed.scrollTop; const img = document.createElement("img"); img.setAttribute("data-media", ""); img.alt = ""; img.src = "data:image/gif;base64,R0lGODlhAQABAAAAACw="; Object.assign(img.style, { position: "absolute", left: x + "px", top: y + "px", width: "120px", height: "60px", zIndex: "0" }); feed.append(img); feed.dispatchEvent(new Event("scroll")); }, [pillBox.x + 10, pillBox.y]);
  await page.waitForTimeout(300);
  ok("a picture under a floating pill marks the pill as a hazard", (await page.locator("header .title").getAttribute("data-hazard")) !== null);
  const alpha = await page.locator("header .title > .glass").evaluate((e) => getComputedStyle(e).backgroundColor);
  ok("its glass fill rises", /0\.9/.test(alpha), alpha);
  await ctx.close();
}
{ // paste a link, storage list
  const { page, ctx } = await open(b, "desktop", "light", "/settings/appearance?sheet=1");
  const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==", "base64");
  await page.route("https://pics.test/**", (r) => r.fulfill({ status: 200, contentType: "image/png", body: png, headers: { "access-control-allow-origin": "*" } }));
  await page.waitForSelector("#u-link");
  await page.locator("#u-link").fill("https://pics.test/forest-light.png");
  await page.getByRole("button", { name: "Use link" }).click();
  await page.waitForSelector(".store");
  ok("a pasted picture link is stored and chosen", (await page.locator(".store li").first().innerText()).includes("forest-light"));
  ok("the storage list shows a size and a total", /KB|MB/.test(await page.locator(".store .tot").innerText()));
  await page.getByRole("button", { name: "Remove" }).first().click();
  await page.waitForTimeout(400);
  ok("Remove deletes it", (await page.locator(".store").count()) === 0);
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);
