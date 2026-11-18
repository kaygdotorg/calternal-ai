// Fetches the default wallpapers at build time into public/wallpapers (gitignored, so never committed; vite copies them into the build output).
// The list is wallpapers.json (photo IDs and photographers, from docs/settings.md). Each photo is downloaded through its Unsplash page's download
// link, then resized by the image host (a 2400 px picture for the wallpaper, 360 px for the picker, and a lighter 2400 px one for the light scheme). Already fetched files are kept; a failed
// download is reported and skipped, so an offline build still works (the app then offers only the generated looks).
// A machine the image host refuses (some dev VMs) takes the files from a cache folder (`CAL_WALLPAPER_CACHE`, default ~/.cache/calternal-ai/wallpapers), which every successful fetch refreshes:
// copy that folder from a machine that fetched them (the browser gate, `npm run e2e:gate`, needs them).
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "public", "wallpapers");
mkdirSync(out, { recursive: true });
const cache = process.env.CAL_WALLPAPER_CACHE ?? join(homedir(), ".cache", "calternal-ai", "wallpapers");
if (existsSync(cache)) cpSync(cache, out, { recursive: true, force: false }); // what is already here is kept; what is missing comes from the cache
const list = JSON.parse(readFileSync(join(here, "..", "wallpapers.json"), "utf8"));
const credits = [];

async function get(url, file) {
  const r = await fetch(url, { signal: AbortSignal.timeout(60000) });
  if (!r.ok) throw new Error(`${r.status} for ${url}`);
  writeFileSync(file, Buffer.from(await r.arrayBuffer()));
}

// The default photograph also gets a tiny pre-blurred rendition per scheme (about 3 KB each): index.html paints it before any script runs, so a first visit never shows a flat ground.
const tokens = JSON.parse(readFileSync(join(here, "..", "..", "..", "packages", "ui", "tokens", "tokens.json"), "utf8"));
const DEFAULT = tokens.appearance["default-wallpaper"];

for (const p of list) {
  const full = join(out, `${p.id}.jpg`), thumb = join(out, `${p.id}-thumb.jpg`), light = join(out, `${p.id}-light.jpg`);
  try {
    if (!existsSync(full) || !existsSync(thumb) || !existsSync(light)) {
      // The download link redirects to the image host with the photo's long name; the host resizes on request.
      const head = await fetch(`https://unsplash.com/photos/${p.id}/download?force=true`, { method: "HEAD", redirect: "follow", signal: AbortSignal.timeout(60000) });
      const base = new URL(head.url);
      for (const k of ["w", "h", "q", "dl"]) base.searchParams.delete(k);
      const sized = (w, q) => { const u = new URL(base); u.searchParams.set("w", String(w)); u.searchParams.set("q", String(q)); u.searchParams.set("fit", "max"); return u.href; };
      await get(sized(1600, 72), full);
      await get(sized(360, 70), thumb);
      // The photographs suit the dark scheme. The light scheme gets a lighter rendition of each, made by the image host (brighter, flatter, a little less saturated),
      // so a light sidebar does not have to be nearly opaque to keep its text readable.
      const lighter = new URL(sized(1600, 72));
      for (const [k, v] of Object.entries({ bri: 35, con: -25, sat: -8 })) lighter.searchParams.set(k, String(v));
      await get(lighter.href, light);
    }
    if (p.id === DEFAULT && (!existsSync(join(out, `${p.id}-blur.jpg`)) || !existsSync(join(out, `${p.id}-blur-light.jpg`)))) {
      const head = await fetch(`https://unsplash.com/photos/${p.id}/download?force=true`, { method: "HEAD", redirect: "follow", signal: AbortSignal.timeout(60000) });
      const base = new URL(head.url);
      for (const k of ["w", "h", "q", "dl"]) base.searchParams.delete(k);
      const tiny = (extra) => { const u = new URL(base); u.searchParams.set("w", "96"); u.searchParams.set("q", "50"); u.searchParams.set("fit", "max"); u.searchParams.set("blur", "40"); for (const [k, v] of Object.entries(extra)) u.searchParams.set(k, String(v)); return u.href; };
      await get(tiny({}), join(out, `${p.id}-blur.jpg`));
      await get(tiny({ bri: 35, con: -25, sat: -8 }), join(out, `${p.id}-blur-light.jpg`));
    }
    credits.push({ id: p.id, name: p.author, author: p.author, link: `https://unsplash.com/photos/${p.id}` });
  } catch (e) {
    console.warn(`wallpapers: skipped ${p.id}: ${e.message}`);
  }
}
if (credits.length) { writeFileSync(join(out, "credits.json"), JSON.stringify(credits, null, 1)); try { cpSync(out, cache, { recursive: true }); } catch { /* the cache is a convenience */ } }
console.log(`wallpapers: ${credits.length} of ${list.length} available`);
if (credits.length < list.length) console.warn(`wallpapers: ${list.length - credits.length} are missing, so the default wallpaper and the picker are incomplete and the browser gate (npm run e2e:gate) will refuse to start. If the image host refuses this machine (a 401 above), copy the folder from a machine that has them and build again:\n  rsync -a ubuntu@calternal-dev-02-std:.cache/calternal-ai/wallpapers/ ~/.cache/calternal-ai/wallpapers/ && npm run build -w apps/web`);
