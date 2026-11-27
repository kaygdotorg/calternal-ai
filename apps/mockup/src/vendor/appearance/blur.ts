/**
 * A blur and a saturation for browsers whose canvas has no `filter` (Safari, on iPhones too): there the filtered draw is silently a plain draw, which left the wallpaper copy sharp and, scaled up, blocky.
 * Three box passes per axis approximate a Gaussian; each pass is a running sum, so the cost is a few operations per pixel whatever the radius. Works in place on RGBA bytes (alpha is left alone).
 */
function boxPass(src: Uint8ClampedArray, dst: Uint8ClampedArray, w: number, h: number, r: number, horizontal: boolean): void {
  const len = horizontal ? w : h, lines = horizontal ? h : w, step = horizontal ? 4 : w * 4, lineStep = horizontal ? w * 4 : 4;
  const win = 2 * r + 1;
  for (let l = 0; l < lines; l++) {
    const base = l * lineStep;
    for (let c = 0; c < 3; c++) {
      const at = (i: number) => src[base + Math.min(len - 1, Math.max(0, i)) * step + c]!; // the edge pixel repeats
      let sum = 0;
      for (let i = -r; i <= r; i++) sum += at(i);
      for (let i = 0; i < len; i++) {
        dst[base + i * step + c] = sum / win;
        sum += at(i + r + 1) - at(i - r);
      }
    }
    for (let i = 0; i < len; i++) dst[base + i * step + 3] = src[base + i * step + 3]!;
  }
}

/** Blurs `data` (w by h RGBA) by about `radius` pixels, in place. */
export function blurRgba(data: Uint8ClampedArray, w: number, h: number, radius: number): void {
  const r = Math.round(radius / Math.sqrt(3)); // three passes of radius r have a spread of about r * sqrt(3)
  if (r < 1) return;
  const tmp = new Uint8ClampedArray(data.length);
  for (let pass = 0; pass < 3; pass++) {
    boxPass(data, tmp, w, h, r, true);
    boxPass(tmp, data, w, h, r, false);
  }
}

/** Scales every pixel's distance from its grey by `amount` (1 leaves it, 1.2 is 120 %), in place. */
export function saturateRgba(data: Uint8ClampedArray, amount: number): void {
  for (let i = 0; i < data.length; i += 4) {
    const y = 0.2126 * data[i]! + 0.7152 * data[i + 1]! + 0.0722 * data[i + 2]!;
    data[i] = y + (data[i]! - y) * amount;
    data[i + 1] = y + (data[i + 1]! - y) * amount;
    data[i + 2] = y + (data[i + 2]! - y) * amount;
  }
}
