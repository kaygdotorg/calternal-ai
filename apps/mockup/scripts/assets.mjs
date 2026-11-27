// Copies the shipped fonts and wallpapers the web app prepares (apps/web `prepare:assets`: scripts/fonts.mjs and scripts/wallpapers.mjs) into public/, so the mockup draws with the same faces and pictures as the app.
import { cpSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
const here = dirname(fileURLToPath(import.meta.url)), web = join(here, "../../web/public"), out = join(here, "../public"), sharedFonts = join(here, "../../../packages/canvas-shell/assets/fonts-extra");
for (const d of ["fonts", "wallpapers"]) {
  if (!existsSync(join(web, d))) { console.error(`apps/web/public/${d} is missing: run \`node apps/web/scripts/fonts.mjs && node apps/web/scripts/wallpapers.mjs\` first.`); process.exit(1); }
  mkdirSync(join(out, d), { recursive: true }); cpSync(join(web, d), join(out, d), { recursive: true });
}
cpSync(sharedFonts, join(out, "fonts-extra"), { recursive: true });
cpSync(sharedFonts, join(out, "fonts"), { recursive: true });
console.log("mockup assets: fonts and wallpapers copied");
