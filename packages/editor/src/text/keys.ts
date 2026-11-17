/**
 * The shortcuts every text surface shares, as data: a text field reads them through `matchKey`, the block editor builds its CodeMirror bindings from the same list (`block/editor.ts`), and both run
 * the same commands (`commands.ts`). Everything CodeMirror binds beyond this list and its plain cursor, delete and history keys is taken off the block editor, so it is never a behaviour a field
 * lacks. Key names follow CodeMirror's (`Mod` is Command on a Mac and Control elsewhere).
 */
import type { CommandId } from "./commands";

/** One shortcut. `lines` bindings only make sense where there are several lines. */
export interface KeySpec { key: string; mac?: string; run: CommandId; lines: boolean }

/** The shared shortcuts. */
export const KEYMAP: KeySpec[] = [
  { key: "Alt-ArrowUp", run: "moveLineUp", lines: true },
  { key: "Alt-ArrowDown", run: "moveLineDown", lines: true },
  { key: "Shift-Alt-ArrowUp", run: "copyLineUp", lines: true },
  { key: "Shift-Alt-ArrowDown", run: "copyLineDown", lines: true },
  { key: "Shift-Mod-k", run: "deleteLine", lines: true },
  { key: "Alt-l", mac: "Ctrl-l", run: "selectLine", lines: true },
  { key: "Mod-]", run: "indentMore", lines: true },
  { key: "Mod-[", run: "indentLess", lines: true },
  { key: "Escape", run: "collapse", lines: false },
];

const isMac = (): boolean => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** The shortcut spec in CodeMirror's notation for this platform, split into a canonical modifier set and a key. */
function parse(spec: string, mac: boolean): { mods: string; key: string } {
  const parts = spec.split("-");
  let key = parts.pop()!;
  if (spec.endsWith("--")) key = "-";
  const mods = new Set<string>();
  for (const p of parts) { if (p === "Mod") mods.add(mac ? "Meta" : "Ctrl"); else if (p === "Cmd") mods.add("Meta"); else if (p) mods.add(p); }
  return { mods: [...mods].sort().join("+"), key };
}

/** The command a keydown triggers, if it matches a shared shortcut (and, for line commands, only when `lines`). */
export function matchKey(e: KeyboardEvent, lines: boolean): CommandId | null {
  const mac = isMac();
  const mods = [e.altKey && "Alt", e.ctrlKey && "Ctrl", e.metaKey && "Meta", e.shiftKey && "Shift"].filter(Boolean).sort().join("+");
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  for (const s of KEYMAP) {
    if (s.lines && !lines) continue;
    const p = parse(mac && s.mac ? s.mac : s.key, mac);
    if (p.mods === mods && (p.key.length === 1 ? p.key.toLowerCase() : p.key) === k) return s.run;
    if (p.mods === mods && p.key.length === 1 && e.code === `Key${p.key.toUpperCase()}`) return s.run; // a letter typed with Alt makes another character on a Mac: the physical key counts
  }
  return null;
}
