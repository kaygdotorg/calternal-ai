/** A code block that scrolls sideways shows it: a fade on the side where there is more (`.mr` more to the right, `.ml` more to the left), kept current as the page changes. One observer for every `pre` in the page, so no app has to remember. */
const SEEN = new WeakSet<HTMLElement>();
function attach(n: HTMLElement) {
  if (SEEN.has(n)) return; SEEN.add(n);
  const f = () => { n.classList.toggle("mr", n.scrollLeft + n.clientWidth < n.scrollWidth - 2); n.classList.toggle("ml", n.scrollLeft > 2); };
  f(); n.addEventListener("scroll", f, { passive: true }); new ResizeObserver(f).observe(n);
}
export function cueCode(): void {
  const scan = () => document.querySelectorAll<HTMLElement>("pre, [data-table-wrap]").forEach(attach);
  scan(); new MutationObserver(scan).observe(document.body, { childList: true, subtree: true });
}
