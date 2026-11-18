/** A Svelte action: calls `more` whenever the node scrolls into (or near) view, so a list fetches its next server page before the person reaches the end of what is loaded. Calls stop when `more` is not given. */
export function reach(node: HTMLElement, more: (() => void) | undefined) {
  let fn = more;
  const io = new IntersectionObserver((e) => { if (e.some((x) => x.isIntersecting)) fn?.(); }, { rootMargin: "400px" });
  io.observe(node);
  return { update(next: (() => void) | undefined) { fn = next; }, destroy() { io.disconnect(); } };
}
