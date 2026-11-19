import { appearance } from "../../appearance/prefs.svelte";
import type { ChromeAppearance } from "./types";

/** The wallpapers the shell paints itself, by name (the three procedural ones CalternalUI has): `auto` and these need no picture sent. */
const NATIVE = new Set(["auto", "dune", "aurora", "ember"]);
const images = new Map<string, string | "pending" | "failed">();
let sentImage = "";
/** Bumped when a picture has been read, so the publisher builds the description again (it reads this). */
export const imageTick = { n: 0 } as { n: number };

/** A picture's bytes as a data URL (the shell takes PNG or JPEG data URLs, once). */
async function dataUrl(url: string): Promise<string> {
  const blob = await (await fetch(url)).blob();
  return await new Promise<string>((res, rej) => { const r = new FileReader(); r.onload = () => res(String(r.result)); r.onerror = () => rej(r.error); r.readAsDataURL(blob); });
}

/**
 * The person's look as the shell paints it (contract/chrome-v1.schema.json): the wallpaper (`auto`, a built-in name, or a library id, and for a library wallpaper the picture itself ONCE as a data URL: the shell blurs it and
 * solves the veils natively), the Blur, Dim and Tint switches and the interface scale. A custom upload or a searched picture lives in the page's storage and has no address, so only the id is sent for those. Reads reactive
 * preferences, so a change republishes; the picture is read in the background and `onImage` is told when it is ready.
 */
export function appearanceOf(onImage: () => void): ChromeAppearance {
  const p = appearance.prefs;
  const id = p.wallpaperId;
  const out: ChromeAppearance = { wallpaper: id, blur: p.materials.blur, dim: p.materials.dim, tint: p.materials.tint, uiSize: p.uiSize / 100 };
  if (id && !NATIVE.has(id) && /^[a-z0-9-]+$/i.test(id)) {
    const have = images.get(id);
    if (have && have !== "pending" && have !== "failed") { if (sentImage !== id) out.image = have; }
    else if (!have) {
      images.set(id, "pending");
      void dataUrl(`${import.meta.env.BASE_URL}wallpapers/${id}.jpg`).then((u) => { images.set(id, u); onImage(); }, () => images.set(id, "failed"));
    }
  }
  return out;
}

/** The picture of `id` has been sent: it is not sent again until the wallpaper changes. */
export function imageSent(id: string | null): void { if (id) sentImage = id; }
