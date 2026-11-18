/**
 * What a plugin gives the palette, as data: phrases that mean a command and commands themselves. The engine does the matching; no plugin code runs while the
 * person types. The shape is the contract the real plugin manifests will carry; the built-in apps register theirs below.
 */

/** A command a plugin offers. `words` are extra phrases that also find it. */
export interface CommandDef {
  /** Stable id; unique across plugins. */
  id: string;
  /** The name shown in the list and matched by the search. */
  label: string;
  /** Where it is from: an app or a settings section, shown as the row's small label. */
  group: string;
  /** Extra words that find the command ("keys", "hotkeys"); they add matches, never rows. */
  words?: string[];
  /** The address it goes to, or an operation name the host runs. */
  go?: string;
  /** A named host operation to run instead of navigating (`help.shortcuts`, `thread.new`, `space.switch:<id>`). */
  op?: string;
}

/** One plugin's contribution. */
export interface PaletteManifest {
  /** The plugin the commands belong to (shown in credits and used to replace its earlier registration). */
  plugin: string;
  /** The commands the plugin offers, as data; nothing runs until one is chosen. */
  commands: CommandDef[];
}

const manifests: PaletteManifest[] = [];

/** Registers a plugin's commands (the same plugin twice replaces the first). */
export function registerManifest(m: PaletteManifest): void {
  const i = manifests.findIndex((x) => x.plugin === m.plugin);
  if (i >= 0) manifests[i] = m;
  else manifests.push(m);
}

/** Every registered command, in registration order; a command id registered twice (the built-in app and the server's manifest for the same plugin) shows once, the first registration winning. */
export const allCommands = (): CommandDef[] => { const seen = new Set<string>(); return manifests.flatMap((m) => m.commands).filter((c) => !seen.has(c.id) && !!seen.add(c.id)); };

/** The prefix of a manifest that came from the server (`plugins.manifests`): they are replaced as a set whenever the list is read again, so a plugin turned off stops offering its commands. */
const REMOTE = "server:";
/** What `plugins.manifests` returns for one enabled plugin: its commands and the Settings sections it provides (each becomes a command that goes to the section). */
export interface ServerManifest { plugin: string; version?: string; commands: CommandDef[]; settings_sections?: { id: string; label: string; go: string }[] }
/** Replaces every server-provided manifest with this list (an empty list removes them). Built-in registrations are never touched. */
export function replaceServerManifests(list: ServerManifest[]): void {
  for (let i = manifests.length - 1; i >= 0; i--) if (manifests[i]!.plugin.startsWith(REMOTE)) manifests.splice(i, 1);
  for (const m of list) manifests.push({ plugin: REMOTE + m.plugin, commands: [...m.commands, ...(m.settings_sections ?? []).map((s) => ({ id: `${m.plugin}:settings:${s.id}`, label: `Settings: ${s.label}`, group: "Settings", go: s.go }))] });
}

registerManifest({
  plugin: "calternal-threads",
  commands: [
    { id: "threads.open", label: "Open Threads", group: "Threads", go: "/threads", words: ["agents", "chat"] },
    { id: "threads.new", label: "New thread", group: "Threads", op: "thread.new", words: ["start thread", "create thread"] },
  ],
});
registerManifest({ plugin: "calternal-notes", commands: [{ id: "notes.open", label: "Open Notes", group: "Notes", go: "/notes" }, { id: "notes.new", label: "New note", group: "Notes", op: "note.new", words: ["create note"] }] });
// The Settings plugin registers a command for every section and every setting itself (apps/settings/registry.ts).
