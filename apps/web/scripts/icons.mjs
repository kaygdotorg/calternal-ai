// Makes the app's PNG icons from public/icons/icon.svg (the manifest and Apple's home-screen icon need PNGs): `node scripts/icons.mjs`. Run once and commit the results; they change only when the icon does.
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
const svg = readFileSync("public/icons/icon.svg", "utf8");
const b = await chromium.launch();
const make = async (file, size, maskable = false) => {
  const p = await b.newPage({ viewport: { width: size, height: size } });
  // A maskable icon keeps its picture inside the middle 80 % (the platform crops the rest), so it is drawn smaller on the full-bleed gradient.
  const inner = maskable ? `<div style="position:absolute;inset:0;background:linear-gradient(135deg,#3b6ea8,#25222a)"></div><div style="position:absolute;inset:10%">${svg}</div>` : svg;
  await p.setContent(`<body style="margin:0;width:${size}px;height:${size}px;position:relative">${maskable ? inner : `<div style="width:${size}px;height:${size}px">${svg.replace("<svg ", `<svg width="${size}" height="${size}" `)}</div>`}</body>`);
  await p.screenshot({ path: `public/icons/${file}`, omitBackground: !maskable });
  await p.close();
};
await make("icon-192.png", 192); await make("icon-512.png", 512); await make("maskable-512.png", 512, true); await make("apple-touch-icon.png", 180);
await b.close();
