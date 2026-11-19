import { appearance } from "../../appearance/prefs.svelte";
import type { ChromeAppearance } from "./types";

/** The wallpapers the shell paints itself, by name (the three procedural ones CalternalUI has): `auto` and these need no picture sent. */
const NATIVE = new Set(["auto", "dune", "aurora", "ember"]);
const images = new Map<string, string | "pending" | "failed">();
/** Bumped when a picture has been read, so the publisher builds the description again (it reads this). */
export const imageTick = { n: 0 } as { n: number };

/** The longest side of the picture sent to the shell, in pixels: it blurs the picture anyway, and the string rides in every description. */
const MAX_SIDE = 960;
/** A picture as a JPEG data URL no larger than `MAX_SIDE` on its longest side (the shell takes PNG or JPEG data URLs). */
async function dataUrl(url: string): Promise<string> {
  const bmp = await createImageBitmap(await (await fetch(url)).blob());
  const k = Math.min(1, MAX_SIDE / Math.max(bmp.width, bmp.height));
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(bmp.width * k)); c.height = Math.max(1, Math.round(bmp.height * k));
  c.getContext("2d")!.drawImage(bmp, 0, 0, c.width, c.height);
  bmp.close();
  return c.toDataURL("image/jpeg", 0.72);
}

/**
 * The person's look as the shell paints it (contract/chrome-v1.schema.json): the wallpaper (`auto`, a built-in name, or a library id, and for a library wallpaper the picture itself as a data URL, the same string in every description: the shell blurs it and
 * solves the veils natively, decoding it again only when the string changes), the Blur, Dim and Tint switches and the interface scale. A custom upload or a searched picture lives in the page's storage and has no address, so only the id is sent for those. Reads reactive
 * preferences, so a change republishes; the picture is read in the background and `onImage` is told when it is ready.
 */
export function appearanceOf(onImage: () => void): ChromeAppearance {
  const p = appearance.prefs;
  const id = p.wallpaperId;
  const out: ChromeAppearance = { wallpaper: id, blur: p.materials.blur, dim: p.materials.dim, tint: p.materials.tint, uiSize: p.uiSize / 100, material: p.material, frost: p.frost };
  if (id && !NATIVE.has(id) && /^[a-z0-9-]+$/i.test(id)) {
    const have = images.get(id);
    if (have && have !== "pending" && have !== "failed") out.image = have; // in every description: leaving it out makes the shell fall back to the named wallpaper
    else if (!have) {
      images.set(id, "pending");
      void dataUrl(`${import.meta.env.BASE_URL}wallpapers/${id}.jpg`).then((u) => { images.set(id, u); onImage(); }, () => images.set(id, "failed"));
    }
  }
  return out;
}
