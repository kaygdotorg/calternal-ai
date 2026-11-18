// The Composer's recogniser: reads the WHOLE phrase and says what it is and what Enter would do. Plain
// rules, no network, built to run on every keystroke (C5, C12: 8 ms on the oldest phone, so it stays
// TypeScript; no WASM, no string crossing). Each app adds its own phrases as a plugin (C11): a plugin
// reads the text, claims the words it understands and fills slots. The core only keeps the books.
//
// The same recogniser reads a checkbox in a note (C22), so it knows nothing of the palette.
import { clockNow, type Now } from "./dates";

/** What the line is. "open" is plain words that match something (Enter opens it); "note" is plain words that don't. */
export type Kind = "task" | "note" | "open" | "contact";
/** What a recognised stretch of the line is, for highlighting behind the text. */
export type SpanType = "date" | "time" | "repeat" | "deadline" | "priority" | "tag" | "person" | "email" | "phone" | "company" | "title";
/** A recognised stretch of the line: `[start, end)` in the typed text, and what it is. */
export type Span = { start: number; end: number; type: SpanType };
/** A repeat ("every 2 weeks", "every monday", "every weekday"): the interval in `unit`s, and optionally the weekday or the weekdays-only rule. */
export type Repeat = { every: number; unit: "day" | "week" | "month"; /** Weekday (0 = Sunday) for "every monday". */ on?: number; /** "every weekday": Monday to Friday. */ weekdays?: boolean };
/** 0 none, 1 low, 2 medium, 3 high, 4 urgent. */
export type Priority = 0 | 1 | 2 | 3 | 4;

/** What the phrases understood. A plugin fills what it knows; the rest stays empty. */
export type Slots = {
  when: string | null;
  time: string | null;
  endTime: string | null;
  deadline: string | null;
  repeat: Repeat | null;
  priority: Priority;
  /** `#errands/calls` without the #. */
  tags: string[];
  /** `@priya` without the @. */
  people: string[];
};

/** The whole reading of one line: what it is (`kind`), the slots the phrases filled, the title that is left, and the spans to highlight. */
export type Parsed = Slots & {
  kind: Kind;
  /** What is left of the words once the recognised ones are taken out, spaces folded, a trailing `on`, `at`, `by`, `for`, `from`, `until`, `in`, `this` or `next` and edge punctuation dropped; the whole line when nothing is left. */
  title: string;
  spans: Span[];
  /** What the phrases voted before any forcing: the kind Enter would have used. */
  guess: Kind | null;
  /** A contact's card, when a plugin read the line as one. */
  extra?: Record<string, unknown>;
};

/** What a plugin sees while it reads one phrase. */
export type Reader = {
  text: string;
  now: Now;
  slots: Slots;
  spans: Span[];
  /** Claim [start, end) for a type; false (and nothing claimed) when some of it is already claimed or the user kept it as text (quotes, backticks, `keep`). */
  claim(start: number, end: number, type: SpanType): boolean;
  /** Runs `fn` for every match of `re` in the text. `re` must carry the `g` flag (a non-global one would match the first hit forever); its `lastIndex` is reset first. */
  each(re: RegExp, fn: (m: RegExpExecArray) => void): void;
  /** Whether a span of the text is already claimed. */
  free(start: number, end: number): boolean;
  /** Where the user is (a plugin can use it to break a tie, C19). */
  here: Here;
  /** A checkbox in a note (C22): only metadata makes it a task, never a verb or the place. */
  checkbox: boolean;
  extra: Record<string, unknown>;
};

/** Where the phrase is being typed: context only ever breaks a tie (C19). */
export type Here = "tasks" | "daily" | "note" | "list" | "other";

/** One app's understanding of plain words (registered with `registerPhrases`): `read` claims words and fills slots; `vote` may say what the line is. Plugins never see each other's words. */
export type Phrases = {
  id: string;
  /** Claims words and fills slots. */
  read?(r: Reader): void;
  /** Says what the line is, when this plugin has a view. The first plugin with one wins. */
  vote?(r: Reader): Kind | null;
};

const plugins: Phrases[] = [];

/** Adds an app's phrases (C11). Same id twice replaces the first. */
export function registerPhrases(p: Phrases): void {
  const i = plugins.findIndex((x) => x.id === p.id);
  if (i >= 0) plugins[i] = p;
  else plugins.push(p);
}

/** How to read a line: forced kind, the clock (`now`), where it is typed (`here`), words to keep as text, and whether it is a checkbox in a note. */
export type Options = {
  /** Forced by the user (the kind picker). */
  forced?: Kind;
  now?: Now;
  here?: Here;
  /** Words the user chose to keep as text (C17): never claimed, whatever they look like. */
  keep?: string[];
  /** Reading a note's checkbox, not the palette (C22). */
  checkbox?: boolean;
};

const emptySlots = (): Slots => ({ when: null, time: null, endTime: null, deadline: null, repeat: null, priority: 0, tags: [], people: [] });

// Text in quotes or backticks is never read: the quickest way to say "keep this as text".
const QUOTED = /"[^"\n]*"|`[^`\n]*`|“[^”\n]*”/g;

/** Reads one line of plain words on the device: no model, no network. Pure for a given `now`; cost is a few passes over the text per keystroke. Anything it cannot read stays in the title, so a failure is "not recognised", never an error. */
export function recognise(text: string, opts: Options = {}): Parsed {
  const now = opts.now ?? clockNow();
  const taken: boolean[] = new Array(text.length).fill(false);
  const spans: Span[] = [];
  const slots = emptySlots();
  const claim = (s: number, e: number, type: SpanType) => {
    for (let i = s; i < e; i++) if (taken[i]) return false;
    for (let i = s; i < e; i++) taken[i] = true;
    spans.push({ start: s, end: e, type });
    return true;
  };
  // Kept words and quoted text are used up before any plugin looks, as untyped claims nobody reports.
  const blocked: boolean[] = new Array(text.length).fill(false);
  const block = (s: number, e: number) => {
    for (let i = s; i < e; i++) blocked[i] = true;
  };
  for (const m of text.matchAll(QUOTED)) block(m.index!, m.index! + m[0].length);
  for (const w of opts.keep ?? []) {
    if (!w) continue;
    const low = text.toLowerCase();
    const k = w.toLowerCase();
    for (let at = low.indexOf(k); at >= 0; at = low.indexOf(k, at + k.length)) block(at, at + k.length);
  }
  const free = (s: number, e: number) => {
    for (let i = s; i < e; i++) if (taken[i] || blocked[i]) return false;
    return true;
  };
  const r: Reader = {
    text,
    now,
    slots,
    spans,
    here: opts.here ?? "other",
    checkbox: !!opts.checkbox,
    extra: {},
    free,
    claim: (s, e, t) => free(s, e) && claim(s, e, t),
    each(re, fn) {
      re.lastIndex = 0;
      let m: RegExpExecArray | null;
      while ((m = re.exec(text))) {
        fn(m);
        if (m[0] === "") re.lastIndex++;
      }
    },
  };

  let guess: Kind | null = null;
  for (const p of plugins) p.read?.(r);
  for (const p of plugins) {
    const v = p.vote?.(r);
    if (v) {
      guess = v;
      break;
    }
  }

  // What is left is the title.
  let title = "";
  for (let i = 0; i < text.length; i++) if (!taken[i]) title += text[i];
  title = title
    .replace(/\s+/g, " ")
    .replace(/\b(on|at|by|for|from|until|in|this|next)\s*$/i, "")
    .replace(/^[\s:,;–—-]+|[\s:,;–—-]+$/g, "");
  if (!title) title = text.trim();

  const kind = opts.forced ?? guess ?? "note";
  return { ...slots, kind, guess, title, spans: spans.sort((a, b) => a.start - b.start), extra: r.extra };
}

/**
 * C22: any metadata turns a checkbox into a task: a date, time, repeat, deadline, priority, #tag or @person.
 * Words alone ("buy milk") do not; the note keeps its plain checkbox.
 */
export function checkboxIsTask(text: string, now?: Now): boolean {
  return recognise(text, { checkbox: true, now }).guess === "task";
}
