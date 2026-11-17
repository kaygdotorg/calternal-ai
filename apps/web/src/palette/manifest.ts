/**
 * What a plugin gives the palette, as data: phrases that mean a command and commands themselves. The engine does the matching; no plugin code runs while the
 * person types. The shape is the contract the real plugin manifests will carry; the built-in apps register theirs below.
 */

/** A command a plugin offers. `words` are extra phrases that also find it. */
export interface CommandDef {
  id: string;
  label: string;
  /** Where it is from: an app or a settings section, shown as the row's small label. */
  group: string;
  words?: string[];
  /** The address it goes to, or an operation name the host runs. */
  go?: string;
  op?: string;
}

/** One plugin's contribution. */
export interface PaletteManifest {
  plugin: string;
  commands: CommandDef[];
}

const manifests: PaletteManifest[] = [];

/** Registers a plugin's commands (the same plugin twice replaces the first). */
export function registerManifest(m: PaletteManifest): void {
  const i = manifests.findIndex((x) => x.plugin === m.plugin);
  if (i >= 0) manifests[i] = m;
  else manifests.push(m);
}

/** Every registered command, in registration order. */
export const allCommands = (): CommandDef[] => manifests.flatMap((m) => m.commands);

registerManifest({
  plugin: "calternal-threads",
  commands: [
    { id: "threads.open", label: "Open Threads", group: "Threads", go: "/threads", words: ["agents", "chat"] },
    { id: "threads.new", label: "New thread", group: "Threads", op: "thread.new", words: ["start thread", "create thread"] },
  ],
});
registerManifest({ plugin: "calternal-notes", commands: [{ id: "notes.open", label: "Open Notes", group: "Notes", go: "/notes" }, { id: "notes.new", label: "New note", group: "Notes", op: "note.new", words: ["create note"] }] });
registerManifest({
  plugin: "calternal-settings",
  commands: [
    { id: "settings.appearance", label: "Appearance", group: "Settings", go: "/settings/appearance", words: ["wallpaper", "theme", "fonts", "ui size", "blur", "dark mode"] },
    { id: "settings.agents", label: "Agents", group: "Settings", go: "/settings/agents", words: ["permission", "streaming", "retry"] },
    { id: "settings.space", label: "Space", group: "Settings", go: "/settings/space", words: ["members", "invites", "roles"] },
    { id: "settings.server", label: "Server", group: "Settings", go: "/settings/server" },
    { id: "settings.shortcuts", label: "Keyboard shortcuts", group: "Settings", op: "help.shortcuts", words: ["keys", "hotkeys"] },
  ],
});
