<script lang="ts">
  // The one seamless background behind both sidebars: the wallpaper (blurred once when chosen, appearance/wallpaper.ts) or, with none, a flat ground. A wash of
  // the scheme's tint lies over it (its strength is derived from the picture's brightness so ink stays legible), then a static grain tile: a 256 px
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

<style>
  .backdrop { position: fixed; inset: 0; z-index: 0; background: var(--wall-side, var(--backdrop)) center / cover no-repeat; }
  .wash { position: absolute; inset: 0; background: rgb(var(--tint) / var(--wash, var(--side-tint-a))); transition: background-color var(--dur-base) var(--ease-out); }
  .grain { position: absolute; inset: 0; opacity: var(--grain-a, 0); background-size: 256px 256px; pointer-events: none; }
</style>
