<script lang="ts">
  // The one seamless background behind both sidebars: the wallpaper (blurred once when chosen, appearance/wallpaper.ts) or, with none, a flat ground. A veil
  // lies over it per sidebar (solved from the pixels under it so ink stays legible), then a static grain tile: a 256 px
  // monochrome noise tile made once, used as a plain image at about 3 % (`--grain-a`, 0 turns it off). The blurred layers are smooth and would band
  // without it, and lossy encoders erase grain baked into a picture. Nothing here is a live filter or an animation.
  import { onMount } from "svelte";
  let tile = $state("");
  onMount(() => {
    // mulberry32: the same noise on every client.
    let a = 0x5eed;
    const rnd = () => { a = (a + 0x6d2b79f5) >>> 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
    const c = document.createElement("canvas");
    c.width = c.height = 256;
    const x = c.getContext("2d")!;
    const img = x.createImageData(256, 256);
    for (let i = 0; i < img.data.length; i += 4) { const v = Math.floor(rnd() * 256); img.data[i] = img.data[i + 1] = img.data[i + 2] = v; img.data[i + 3] = 255; }
    x.putImageData(img, 0, 0);
    tile = c.toDataURL("image/png");
  });
</script>

<div class="backdrop" aria-hidden="true"><div class="wash"></div><div class="grain" style:background-image={tile ? `url(${tile})` : "none"}></div></div>
<!-- Safari 26+ tints its status bar and toolbar from a fixed box at the screen's edge and cannot read a colour out of a picture: two thin solid strips in the solved top colour (`--bars`, shell/chrome.ts) at the top and bottom edges give it one. -->
<div class="bars bars-t" aria-hidden="true"></div><div class="bars bars-b" aria-hidden="true"></div>

<style>
  .bars { position: fixed; left: 0; right: 0; height: 3px; z-index: 0; pointer-events: none; background: var(--bars, var(--paper)); }
  .bars-t { top: 0; } .bars-b { bottom: 0; }
  .backdrop { position: fixed; inset: 0; z-index: 0; background: var(--wall-side, var(--wall-fallback, var(--backdrop))) center / cover no-repeat; }
  /* ONE continuous background: the wallpaper under a single veil (appearance/veil.ts, solved once from the pixels under both sidebars) spans the whole window. Both sidebars are drawn on it; the middle card is the only
     thing on top, so nothing else paints in the gap beside the card or behind its rounded corners (there used to be a veil per sidebar that stopped at the card's edge, which left the corners bare). */
  .wash { position: absolute; inset: 0; background: var(--veil-l, linear-gradient(rgb(var(--tint) / 0.5) 0 0)); }
  /* The picture and its veil are painted under the status bar, the notch and the home indicator (the page is `viewport-fit=cover`); the content is what the shell pads. On a home-screen app that is laid out short (shell/chrome.ts) the backdrop is as tall as the screen. */
  :global(html[data-home-screen]) .backdrop { bottom: auto; height: var(--vh, 100%); }
  .grain { position: absolute; inset: 0; opacity: var(--grain-a, 0); background-size: 256px 256px; pointer-events: none; }
</style>
