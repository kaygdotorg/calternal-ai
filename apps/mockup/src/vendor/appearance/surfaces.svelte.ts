/**
 * Which scheme's ink each surface uses right now, as the veil solver decided (appearance/veil.ts): null follows the page's scheme. A sidebar over a very dark
 * picture in the light scheme, say, flips to light text. The shell puts it on the surface as `data-ink`, which the generated tokens turn into that scheme's colours.
 */
export const surfaces = $state<{ left: "light" | "dark" | null; right: "light" | "dark" | null }>({ left: null, right: null });
