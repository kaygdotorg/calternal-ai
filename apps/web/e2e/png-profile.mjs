// Mean brightness of a vertical strip of a screenshot, row by row: the profile of a fade or a band. Usage: node png-profile.mjs <png> <x0> <x1> [step]  (coordinates in the picture's own pixels)
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
const [png, x0, x1, step = "10"] = process.argv.slice(2);
const b = await chromium.launch(); const page = await b.newPage();
const r = await page.evaluate(async ([b64, x0, x1, step, HI]) => {
  const img = new Image(); img.src = "data:image/png;base64," + b64; await img.decode();
  const cv = document.createElement("canvas"); cv.width = img.width; cv.height = img.height; const c = cv.getContext("2d", { willReadFrequently: true }); c.drawImage(img, 0, 0);
  const d = c.getImageData(x0, 0, x1 - x0, img.height).data, w = x1 - x0, out = [];
  const hi = HI; for (let y = 0; y < img.height; y++) { let a = 0, m = 0; for (let x = 0; x < w; x++) { const i = (y * w + x) * 4, v = (d[i] + d[i + 1] + d[i + 2]) / 3; a += v; if (v > m) m = v; } out.push(hi ? m : a / w); }
  let worst = 0, at = 0; for (let y = 1; y < out.length; y++) if (Math.abs(out[y] - out[y - 1]) > worst) { worst = Math.abs(out[y] - out[y - 1]); at = y; }
  return { h: img.height, w: img.width, profile: out.filter((_, i) => i % +step === 0).map((v) => +v.toFixed(1)), worst: +worst.toFixed(2), at };
}, [readFileSync(png).toString("base64"), +x0, +x1, +step, process.env.HI === "1" ? 1 : 0]);
console.log(JSON.stringify(r));
await b.close();
