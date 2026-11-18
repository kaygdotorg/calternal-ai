/**
 * Keyboard shortcuts: one registry of commands, each with a default chord the person can change, a help sheet that lists them (Cmd or Ctrl + /, or ?),
 * and one global handler. Brought across from the existing app's `shortcuts.ts` (mockup) with the same rules:
 *   - the most common convention wins and browser-owned combinations are not used (Cmd+L, Cmd+T, Cmd+1 and so on stay the browser's);
 *   - matching is by physical key (`KeyboardEvent.code`), so a chord works on any keyboard layout;
 *   - ⌘ on a Mac and Ctrl elsewhere are one modifier, `Mod` (never both: Ctrl+Cmd belongs to the system);
 *   - a plain-letter chord (no modifier) never fires while the person is typing in a field;
 *   - every command is also reachable from a bar, a menu or the palette, so a shortcut is never the only way.
 * A chord is stored as text: modifiers in the order Mod, Alt, Shift, then the key's `code` ("Mod+Shift+Backslash"). Changes are kept in localStorage here;
 * the server will keep them per account.
 */

/** A chord such as `Mod+Slash`, `Alt+ArrowDown` or `Shift+Slash`; null means unassigned. */
export type Chord = string | null;

/** One command the person can run. */
export interface Command {
  /** Stable id; the key the person's remapping is saved under. */
  id: string;
  /** The name shown in the help sheet and the palette. */
  label: string;
  /** The help sheet section it is listed in. */
  group: "Navigation" | "Threads" | "Canvas" | "Help";
  /** Where it applies, in plain words, shown in the help sheet. */
  where: string;
  /** The default chord. */
  chord: Chord;
  /** Runs it. Returns false when it does not apply right now (the key then does nothing and the browser may use it). */
  run: () => boolean | void;
}

/** Whether this device uses ⌘ as its command key (Mac, iPhone, iPad); decides `Mod` and the symbols shown. Read once at load, so it is false when rendered without a browser. */
export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

const KEY = "cal.shortcuts";
const SYMBOL: Record<string, string> = { Slash: "/", Backslash: "\\", Comma: ",", Period: ".", Semicolon: ";", Quote: "'", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backquote: "`", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", Escape: "esc", Enter: "↵", Tab: "⇥", Space: "Space", Backspace: "⌫" };

/** The chord for a key event, or null for a lone modifier. */
export function chordOf(e: KeyboardEvent): Chord {
  if (/^(Meta|Control|Alt|Shift)(Left|Right)$/.test(e.code) || !e.code) return null;
  const mod = isMac ? e.metaKey : e.ctrlKey;
  if (isMac && e.ctrlKey && !e.metaKey && !e.altKey) return null; // Ctrl+key is the system's on a Mac
  return [mod ? "Mod" : "", e.altKey ? "Alt" : "", e.shiftKey ? "Shift" : "", e.code].filter(Boolean).join("+");
}

/** A chord as this platform writes it: ⌘⌥⇧ and symbols on a Mac, "Ctrl+Alt+Shift+" words elsewhere. */
export function show(c: Chord): string {
  if (!c) return "Not set";
  const parts = c.split("+");
  const key = parts.pop()!;
  const k = SYMBOL[key] ?? key.replace(/^Key/, "").replace(/^Digit/, "");
  const mods = parts.map((p) => (p === "Mod" ? (isMac ? "⌘" : "Ctrl") : p === "Alt" ? (isMac ? "⌥" : "Alt") : isMac ? "⇧" : "Shift"));
  return isMac ? [...mods, k].join("") : [...mods, k].join("+");
}

/** The `aria-keyshortcuts` form of a chord. */
export const ariaKeys = (c: Chord): string => (c ? c.split("+").map((p) => (p === "Mod" ? (isMac ? "Meta" : "Control") : SYMBOL[p] ?? p.replace(/^Key/, "").replace(/^Digit/, ""))).join("+") : "");

const typing = (t: EventTarget | null): boolean => {
  const el = t as HTMLElement | null;
  return !!el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));
};

class Shortcuts {
  commands = $state<Command[]>([]);
  /** The person's overrides by command id (null = unassigned). */
  overrides = $state<Record<string, Chord>>(JSON.parse(typeof localStorage !== "undefined" ? (localStorage.getItem(KEY) ?? "{}") : "{}"));
  /** The command currently waiting for a new chord in the help sheet. */
  capturing = $state<string | null>(null);
  /** A plain-language problem with the last attempted change, if any. */
  problem = $state("");

  /** Replaces the registered commands (the app registers them once it has what they act on). */
  register(cs: Command[]): void {
    this.commands = cs;
  }

  /** The chord a command currently has. */
  chordFor(id: string): Chord {
    const c = this.commands.find((x) => x.id === id);
    return id in this.overrides ? this.overrides[id]! : (c?.chord ?? null);
  }

  /** The command that owns a chord, if any (other than `except`). */
  owner(chord: Chord, except?: string): Command | undefined {
    return chord ? this.commands.find((c) => c.id !== except && this.chordFor(c.id) === chord) : undefined;
  }

  /** Assigns a chord, refusing a clash with another command (said in plain words). Returns whether it was applied. */
  set(id: string, chord: Chord): boolean {
    const clash = this.owner(chord, id);
    if (clash) {
      this.problem = `${show(chord)} is already ${clash.label.toLowerCase()}. Choose another, or clear that one first.`;
      return false;
    }
    this.problem = "";
    this.overrides = { ...this.overrides, [id]: chord };
    this.save();
    return true;
  }
  /** Back to a command's default. */
  reset(id: string): void {
    const { [id]: _drop, ...rest } = this.overrides;
    this.overrides = rest;
    this.problem = "";
    this.save();
  }
  /** Back to every default. */
  resetAll(): void {
    this.overrides = {};
    this.problem = "";
    this.save();
  }
  private save(): void {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.overrides));
    } catch {
      /* private mode */
    }
  }

  /** The one global key handler. Call once; it follows the registry. */
  start(): void {
    addEventListener("keydown", (e) => {
      if (this.capturing) return this.capture(e);
      if (e.defaultPrevented || e.isComposing) return;
      const c = chordOf(e);
      if (!c) return;
      const plain = !/Mod|Alt/.test(c) && !c.startsWith("Shift+") ? true : false;
      const cmd = this.commands.find((x) => this.chordFor(x.id) === c);
      if (!cmd) return;
      // A chord with no Mod or Alt (a plain key, or Shift+key such as ?) never fires while typing.
      if (!/Mod|Alt/.test(c) && typing(e.target)) return;
      void plain;
      if (cmd.run() !== false) e.preventDefault();
    });
  }

  private capture(e: KeyboardEvent): void {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") return void (this.capturing = null);
    const c = chordOf(e);
    if (!c) return;
    const id = this.capturing!;
    if (this.set(id, c)) this.capturing = null;
  }
}

/** The shared shortcut state. */
export const shortcuts = new Shortcuts();
