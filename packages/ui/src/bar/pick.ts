// Which mode the bar shows (#268, #269), in one place. Plain data in, a key out, so it is tested
// without a browser.
//
// The rule, in order:
// 1. A question waiting for an answer (merging a tag, #101).
// 2. A toast, unless a question is waiting (`toast-<id>`).
// 3. An open sheet (`sheet-insert`, `style`, `move`, `link`, …).
// 4. What you're doing in the text: blocks selected (`block`), words selected (`text`).
// 5. Typing on a phone, the caret in the text: the text bar (`text`), right above the keyboard.
// 6. A note open, the keyboard down or nothing focused: the note tools (`tools`).
// 7. Otherwise the tabs.

export type BarState = {
  /** A question is waiting. */
  ask: boolean;
  /** The toast showing now, if any. */
  toast: string | null;
  /** The open sheet's key, if any. */
  sheet: string | null;
  /** The editing mode: "idle", or "text"/"block" (or a mode's own sheet, such as "link"). */
  mode: string;
  /** On a phone, the caret is in the text and no blocks are selected. */
  typing: boolean;
  /** A note is open and its tools belong in the bar. */
  tools: boolean;
};

/** The key without a toast: what the bar shows under it, and what it goes back to. */
export function pickBase(s: BarState): string {
  if (s.ask) return "ask";
  if (s.sheet) return s.sheet;
  if (s.mode !== "idle") return s.mode;
  if (s.typing) return "text";
  return s.tools ? "tools" : "tabs";
}

export function pickBarKey(s: BarState): string {
  const base = pickBase(s);
  return base !== "ask" && s.toast ? `toast-${s.toast}` : base;
}
