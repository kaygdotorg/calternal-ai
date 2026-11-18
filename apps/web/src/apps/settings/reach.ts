/**
 * A Svelte action: while the node is in (or within 400 px of) view, calls `more` again and again until it leaves view or the node goes away, so a list fetches its next server page before the person reaches the end of
 * what is loaded, and keeps going when a page adds nothing visible (a page of used invitations under a filter that shows only open ones). `more` returns when its page is in.
 */
export function reach(node: HTMLElement, more: (() => Promise<void>) | undefined) {
  let fn = more, visible = false, running = false;
  const pump = async () => {
    if (running) return;
    running = true;
    while (visible && fn) { await fn(); await new Promise((r) => setTimeout(r, 30)); }
    running = false;
  };
  const io = new IntersectionObserver((e) => { visible = e.some((x) => x.isIntersecting); if (visible) void pump(); }, { rootMargin: "400px" });
  io.observe(node);
  return { update(next: (() => Promise<void>) | undefined) { fn = next; }, destroy() { visible = false; io.disconnect(); } };
}
