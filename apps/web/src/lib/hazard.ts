/**
 * Pill-contrast hazards: a floating pill is glass over whatever scrolls beneath it. Text under a pill is calm, but a picture or a video under it can fight the pill's own
 * glyphs. While a list scrolls, any floating bar element that overlaps a `[data-media]` element gets `data-hazard`, and the glass layer raises its fill (glass.css).
 * Cost: one pass per animation frame while scrolling (a handful of rectangles: the pills and the media currently in the rendered window); nothing when idle.
 */
export function hazards(list: HTMLElement, bars: () => HTMLElement[]) {
  let raf = 0;
  const check = () => {
    raf = 0;
    const media = [...list.querySelectorAll<HTMLElement>("[data-media]")].map((m) => m.getBoundingClientRect());
    for (const bar of bars()) {
      const r = bar.getBoundingClientRect();
      const hit = media.some((m) => m.bottom > r.top && m.top < r.bottom && m.right > r.left && m.left < r.right);
      if (hit !== bar.hasAttribute("data-hazard")) bar.toggleAttribute("data-hazard", hit);
    }
  };
  const on = () => { if (!raf) raf = requestAnimationFrame(check); };
  list.addEventListener("scroll", on, { passive: true, capture: true }); // scroll does not bubble: capture reaches the list inside
  on();
  return () => { list.removeEventListener("scroll", on, { capture: true }); cancelAnimationFrame(raf); for (const b of bars()) b.removeAttribute("data-hazard"); };
}
