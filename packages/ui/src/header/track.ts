/**
 * The header tracker, ported from the existing app's `chrome/header/title.ts` and generalised to DOM headings: as a scrolling page's title and then each of its
 * section headings pass the header line, `report` says which one is the current one, so the header can show it in a pill. The page marks its big title with
 * `data-chrome-title` (optionally its text as the value, else the element's text) and each section heading with `data-chrome-heading`.
 *
 * Cost: no scroll handler at all. One IntersectionObserver with a root margin that moves the root's top edge down to the header line tells us when an
 * element crosses it, and `scrollend` covers a flick that skipped past some; nothing reads layout while scrolling (a handful of rects are read once per crossing or stop). A MutationObserver (coalesced to one frame) notices headings that come and go.
 */

/** What the header shows right now. */
export interface HeaderState {
  /** The line to show; empty while the page's own title is still readable. */
  text: string;
  /** Changes whenever the line means something else, even when two headings read the same. */
  key: string;
  /** True when the change should happen at once: the first report and reduced motion. */
  instant: boolean;
}

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

/** The page's title fades out while the header says it (opacity only; it keeps its space), so the two are never seen together. */
function fadeTitle(el: HTMLElement, away: boolean, instant: boolean): void {
  el.style.transition = instant ? "none" : "opacity var(--dur-fast) var(--ease-out)";
  el.style.opacity = away ? "0" : "";
}

/**
 * Follows `scroller` (the element that scrolls) and reports the header's line. `edge` is how far down the header reaches, in pixels from the scroller's top.
 * Returns the way to stop. Safe on a page with no marked title or headings: it reports an empty line once.
 */
export function trackHeader(scroller: HTMLElement, edge: number, report: (s: HeaderState) => void): () => void {
  const gone = new Map<Element, boolean>();
  let first = true, shownKey = "\0", faded: HTMLElement | null = null;
  const marked = () => [...scroller.querySelectorAll<HTMLElement>("[data-chrome-title], [data-chrome-heading]")];
  const textOf = (el: HTMLElement) => (el.dataset.chromeTitle || el.dataset.chromeHeading || el.textContent || "").trim();

  function publish(): void {
    let cur: HTMLElement | null = null;
    for (const el of marked()) if (gone.get(el)) cur = el; // document order: the last one that has gone up is the current one
    const key = cur ? (cur.hasAttribute("data-chrome-title") ? "title" : `h:${textOf(cur)}`) : "";
    const title = scroller.querySelector<HTMLElement>("[data-chrome-title]");
    if (title && (title !== faded || first)) { if (faded && faded !== title) fadeTitle(faded, false, true); faded = title; }
    if (faded) fadeTitle(faded, !!cur && cur === faded, first || reduced());
    if (key === shownKey) return;
    shownKey = key;
    report({ text: cur ? textOf(cur) : "", key, instant: first || reduced() });
    first = false;
  }

  /** Whether `el` has gone up past the header line, from where it is now (one read per element; headings are few). */
  function isGone(el: Element, line: number): boolean {
    const r = el.getBoundingClientRect();
    return el.hasAttribute("data-chrome-title") ? r.top < line && (r.bottom - line) / Math.max(1, r.height) < 0.99 : r.bottom <= line;
  }
  /** Re-derives every element from where it is: an observer only speaks when an element crosses a threshold, and a fast flick can carry one from below the line to above it between two looks. */
  function recompute(): void {
    const line = scroller.getBoundingClientRect().top + edge;
    for (const el of watched) gone.set(el, isGone(el, line));
    publish();
  }
  // The observer says when something crosses the line (no scroll handler, no per-frame work); `scrollend` says when a flick that skipped past several has stopped. Both read layout once, not per frame.
  const io = new IntersectionObserver(recompute, { root: scroller, rootMargin: `${-edge}px 0px 0px 0px`, threshold: [0, 0.99] });
  scroller.addEventListener("scrollend", recompute, { passive: true });

  const watched = new Set<Element>();
  function scan(): void {
    const now = new Set<Element>(marked());
    for (const el of now) if (!watched.has(el)) { watched.add(el); io.observe(el); }
    for (const el of [...watched]) if (!now.has(el)) { watched.delete(el); gone.delete(el); io.unobserve(el); }
    recompute();
  }
  let raf = 0;
  const mo = new MutationObserver(() => { raf ||= requestAnimationFrame(() => { raf = 0; scan(); }); });
  mo.observe(scroller, { childList: true, subtree: true });
  scan();
  if (!watched.size) report({ text: "", key: "", instant: true });

  return () => {
    io.disconnect(); mo.disconnect(); cancelAnimationFrame(raf); scroller.removeEventListener("scrollend", recompute);
    if (faded) fadeTitle(faded, false, true);
  };
}
