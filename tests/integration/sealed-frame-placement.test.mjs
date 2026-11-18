// Browser isolation fixture for the frame placement contract. This does not mount a product renderer.
import assert from "node:assert/strict";
import test from "node:test";
import { launch } from "../../apps/web/e2e/lib.mjs";

const stripes = [
  "repeating-linear-gradient(90deg, red 0px 20px, blue 20px 40px)",
  "repeating-linear-gradient(90deg, yellow 0px 20px, cyan 20px 40px)",
];

test("a host-drawn opaque card isolates a sealed frame from the surrounding backdrop", async () => {
  const browser = await launch();
  try {
    const page = await browser.newPage({
      viewport: { width: 500, height: 260 },
      deviceScaleFactor: 1,
    });
    await page.setContent(`
      <style>
        body { margin: 0; }
        #chrome { position: absolute; left: 10px; top: 40px; width: 60px; height: 100px; background: #634d74; }
        #card { position: absolute; left: 100px; top: 30px; width: 320px; height: 200px; overflow: hidden; }
        iframe { display: block; width: 100%; height: 100%; border: 0; }
      </style>
      <main id="card"><iframe title="Plugin fixture" sandbox="allow-scripts"></iframe></main>
      <aside id="chrome"></aside>
    `);
    await page.locator("iframe").evaluate((frame) => {
      frame.srcdoc = `
        <style>
          html, body { margin: 0; background: transparent; }
          #probe { position: fixed; inset: -120px; backdrop-filter: blur(20px); }
        </style>
        <div id="probe"></div>
      `;
    });
    const probe = page.frameLocator("iframe").locator("#probe");
    await probe.waitFor();
    const chrome = page.locator("#chrome");
    const chromeBefore = await chrome.screenshot();

    async function framePixels(backdrop, backing) {
      await page.evaluate(async ([backdrop, backing]) => {
        document.documentElement.style.background = backdrop;
        document.getElementById("card").style.background = backing;
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      }, [backdrop, backing]);
      return page.screenshot({ clip: { x: 150, y: 90, width: 200, height: 60 } });
    }

    const exposedA = await framePixels(stripes[0], "transparent");
    const exposedB = await framePixels(stripes[1], "transparent");
    assert.notDeepEqual(exposedA, exposedB, "the transparent control must expose the host backdrop");

    const isolatedA = await framePixels(stripes[0], "#3b4350");
    const isolatedB = await framePixels(stripes[1], "#3b4350");
    assert.deepEqual(isolatedA, isolatedB, "changing the host backdrop must not change pixels inside the frame");
    assert.deepEqual(await chrome.screenshot(), chromeBefore, "oversized frame content must stay inside the card");
  } finally {
    await browser.close();
  }
});
