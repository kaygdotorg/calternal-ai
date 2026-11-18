/**
 * The shared header line: one store, one action. A scrolling view puts `use:chromeTrack` on the element that scrolls and marks its big title (`data-chrome-title`)
 * and section headings (`data-chrome-heading`); the shell's `HeaderPill` shows `chromeHeader.text` beside the sidebar button. The same mechanism serves every
 * scrolling card (Settings pages, notes, a thread, a plugin's view). A view whose title is always the header's line (a thread) sets `pinned` instead.
 */
import { cssPx } from "../lib/css";
import { trackHeader, type HeaderState } from "./track";

class ChromeHeader {
  /** The tracked line; empty while the page's own title is readable. */
  text = $state("");
  /** Changes with the line's meaning (drives the swap). */
  key = $state("");
  /** Whether the last change should show at once (first report, reduced motion). */
  instant = $state(true);
  /** A line the view always wants shown (a thread's title); it wins over the tracked one while set. */
  pinned = $state("");
  /** The line the pill shows. */
  get line(): string { return this.pinned || this.text; }
}

/** The header line every scrolling card reports to and the shell draws. */
export const chromeHeader = new ChromeHeader();

let owner: HTMLElement | null = null;

/** Svelte action for the element that scrolls: tracks its marked title and headings into `chromeHeader` and clears it when the view goes. `edge` defaults to the header's height. */
export function chromeTrack(node: HTMLElement, opts?: { edge?: number }): { destroy(): void } {
  owner = node;
  const edge = opts?.edge ?? cssPx("--safe-top") + cssPx("--bar-h");
  const stop = trackHeader(node, edge, (s: HeaderState) => { if (owner !== node) return; chromeHeader.text = s.text; chromeHeader.key = s.key; chromeHeader.instant = s.instant; });
  return { destroy() { stop(); if (owner === node) { owner = null; chromeHeader.text = ""; chromeHeader.key = ""; } } };
}
