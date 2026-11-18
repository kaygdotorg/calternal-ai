/**
 * Says where a scrolling box has more to show: sets `data-more-start` and `data-more-end` (along the chosen axis) while the content continues past that edge, on scroll and when the box or its children change size. The style draws the cue (a soft fade at that
 * edge, the scrollbar made visible), so a box that continues never looks like it ends. Used by Menu (vertical) and the message text's code blocks (horizontal). Cost: one scroll listener (passive) and one ResizeObserver, a few attribute writes only when a value changes.
 */
export function scrollCue(node: HTMLElement, axis: "x" | "y" = "y") {
  const set = () => {
    const pos = axis === "x" ? node.scrollLeft : node.scrollTop, size = axis === "x" ? node.clientWidth : node.clientHeight, total = axis === "x" ? node.scrollWidth : node.scrollHeight;
    node.toggleAttribute("data-more-start", pos > 1);
    node.toggleAttribute("data-more-end", pos + size < total - 1);
  };
  set();
  node.addEventListener("scroll", set, { passive: true });
  const ro = new ResizeObserver(set);
  ro.observe(node);
  for (const c of node.children) ro.observe(c);
  return { destroy() { node.removeEventListener("scroll", set); ro.disconnect(); } };
}
