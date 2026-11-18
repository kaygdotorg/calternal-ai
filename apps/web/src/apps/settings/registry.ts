/**
 * Every setting is declared here as data: a stable id `<plugin>.<setting>` (the setting's name for addresses, the palette and agents: `/settings/<section>/<id>`),
 * its label (a noun phrase), help text, the control it needs, and how it reads and writes. Pages are lists of groups of these; nothing else knows what a setting is.
 * Follows docs/ui-craft-and-settings-study.md section 11: one decision per row, one control per row, no Save button, a Danger area last on admin pages.
 * On fixtures: settings of things that exist (appearance, streaming) are wired to them; the rest keep their value in `values`.
 */
import type { Component } from "svelte";
import type { IconName } from "@calternal/ui";
import { appearance } from "../../appearance/prefs.svelte";
import { registerManifest } from "../../palette/manifest";
import { values } from "./store.svelte";
import { navigate } from "../../lib/route.svelte";
import { shell } from "../../shell/shell.svelte";
import { spaces } from "../../shell/spaces.svelte";
import { devToken, serverMode } from "../../server/client";
import { caps } from "../../shell/capabilities.svelte";

/** How a setting is edited: a switch, a choice, a field, a button, a drill-in row, or a whole panel of its own that loads on demand. Adding a kind means one branch in SettingRow and nothing else. */
export type Control =
  | { type: "switch" }
  | { type: "checkbox" }
  | { type: "segmented"; options: { id: string; label: string }[] }
  | { type: "popup"; options: { id: string; label: string }[] }
  | { type: "text"; placeholder?: string }
  | { type: "number"; min: number; max: number; unit?: string }
  | { type: "slider"; min: number; max: number; unit?: string }
  /** A slider with a few fixed positions, named; the value is the position's index. */
  | { type: "steps"; steps: string[] }
  /** A list the person puts in order (Move up, Move down); the value is the ids in order. */
  | { type: "order" }
  | { type: "scheme" }
  | { type: "presets" }
  | { type: "button"; text: string; danger?: boolean; run: () => void }
  | { type: "drill"; summary: () => string; run: () => void }
  | { type: "info"; text: () => string }
  /** A whole control of its own, full width (a list, a flow): its code loads when the page is opened, so it is not in the first load. */
  | { type: "custom"; load: () => Promise<{ default: Component<any> }> };

/** One setting. `id` is `<plugin>.<setting>` and never changes. */
export interface Setting {
  /** Stable `<plugin>.<setting>` id; the setting's address, its palette entry and what agents name it. Never changes. */
  id: string;
  /** The row's label, a noun phrase. */
  label: string;
  /** One line under the label saying what the choice does. */
  help?: string;
  /** How the row is edited. */
  control: Control;
  /** Reads the current value (reactive); absent for rows that only act. */
  get?: () => unknown;
  /** Writes a new value; applied at once (there is no Save). */
  set?: (v: unknown) => void;
}
/** A card of rows with an optional header; `danger` puts it last, under the Danger area header. */
export interface Group { title?: string; settings: Setting[]; danger?: boolean }
/** One Settings page: its address id (`/settings/<id>`), sidebar name and icon, a one-line purpose, and its groups. */
export interface Page { id: string; name: string; icon: IconName; blurb: string; groups: Group[] }
/** A sidebar section: `user` (this person), `space` (the current space) or `server` (Admin). Which sections and pages exist depends on the source and the capabilities, so read them through `visibleSections`. */
export interface Section { id: "user" | "space" | "server"; name: string; pages: Page[] }

const fx = <T,>(id: string, def: T): Pick<Setting, "get" | "set"> => ({ get: () => values.get<T>(id, def), set: (v) => values.set(id, v) });
const A = appearance;
const ap = (f: (p: typeof A.prefs) => void) => A.change(f);
/** Server mode with a cookie session: ends it on the server and reloads into the sign-in screen. Fixtures only explain. */
const signOutAction = (): void => {
  if (!serverMode() || devToken()) return confirmLater("Sign out")();
  void import("../../server/auth").then((a) => a.signOut()).then(() => location.replace(import.meta.env.BASE_URL), () => alert("Could not sign out. Check the connection and try again."));
};
const confirmLater = (what: string) => () => alert(`${what}\n\nThis is a fixture: nothing is changed. The real action asks for confirmation (and a passkey when it changes access).`);

const appearancePage: Page = {
  id: "appearance", name: "Appearance", icon: "image", blurb: "How the app looks. Everything applies at once.",
  groups: [
    { title: "Look", settings: [
      { id: "appearance.scheme", label: "Colour scheme", help: "Auto follows the system.", control: { type: "scheme" }, get: () => A.scheme, set: (v) => A.setScheme(v as never) },
      { id: "appearance.preset", label: "Preset", help: "A preset sets the wallpaper and the colours together; each stays adjustable.", control: { type: "presets" } },
      { id: "appearance.wallpaper", label: "Wallpaper and fonts", help: "Choose a photograph, upload your own, search Unsplash, pick fonts.", control: { type: "drill", summary: () => A.wallpaper?.name ?? "None", run: () => navigate("/settings/appearance?sheet=1") } },
    ] },
    { title: "Materials", settings: [
      { id: "appearance.blur", label: "Blur", help: "Frosts the picture behind the sidebars and the card.", control: { type: "checkbox" }, get: () => A.prefs.materials.blur, set: (v) => ap((p) => (p.materials.blur = !!v)) },
      { id: "appearance.dim", label: "Dim", help: "A neutral veil over the picture so text stays calm.", control: { type: "checkbox" }, get: () => A.prefs.materials.dim, set: (v) => ap((p) => (p.materials.dim = !!v)) },
      { id: "appearance.tint", label: "Tint", help: "A veil in the colour scheme's paper, with a whisper of the picture's hue.", control: { type: "checkbox" }, get: () => A.prefs.materials.tint, set: (v) => ap((p) => (p.materials.tint = !!v)) },
      { id: "appearance.clarity", label: "Clear to tinted", help: "Five steps from Clear (the most wallpaper shows through the sidebars and the card) to Tinted (the least), as in Apple's clear and tinted glass. Balanced is the readable default; Clear can fall under the readability floor but never changes the surface's light or dark. Starts from the system's reduced-transparency and contrast settings.", control: { type: "steps", steps: ["Clear", "Light", "Balanced", "Dense", "Tinted"] }, get: () => Math.round(A.clarity * 4), set: (v) => A.setClarityStep(Math.min(4, Math.max(0, Math.round(v as number))) / 4) },
    ] },
    { title: "Size and colour", settings: [
      { id: "appearance.ui-size", label: "UI size", help: "One scale for type, spacing and controls together.", control: { type: "slider", min: 80, max: 150, unit: " %" }, get: () => A.prefs.uiSize, set: (v) => ap((p) => (p.uiSize = v as number)) },
      { id: "appearance.accent", label: "Accent", help: "From the wallpaper, or one colour of your own.", control: { type: "segmented", options: [{ id: "wallpaper", label: "From wallpaper" }, { id: "custom", label: "Custom" }] }, get: () => (A.prefs.accent === "theme" ? "wallpaper" : A.prefs.accent), set: (v) => ap((p) => (p.accent = v as never)) },
      { id: "appearance.wordmark", label: "Wordmark", help: "The name at the top of the sidebar.", control: { type: "text", placeholder: "Calternal" }, get: () => A.prefs.brand, set: (v) => ap((p) => (p.brand = String(v).slice(0, 24))) },
    ] },
    { title: "Chat", settings: [
      { id: "appearance.chat-width", label: "Chat layout", help: "Wide: messages and the composer use the whole card, yours on the right, the agent's from the left. Centred: a readable column.", control: { type: "segmented", options: [{ id: "wide", label: "Wide" }, { id: "centered", label: "Centred" }] }, get: () => A.prefs.chatWidth, set: (v) => ap((p) => (p.chatWidth = v === "centered" ? "centered" : "wide")) },
    ] },
    { title: "Motion", settings: [
      { id: "appearance.motion", label: "Animations", help: "Follows the system's reduce-motion setting; this switch turns them off as well.", control: { type: "switch" }, get: () => A.prefs.motion, set: (v) => ap((p) => (p.motion = !!v)) },
    ] },
    { title: "Layout", settings: [
      { id: "appearance.app-order", label: "Apps", help: "The order of the apps in the switcher under the card (and the bottom bar on a phone). Move one up or down.", control: { type: "order" }, get: () => A.prefs.appOrder, set: (v) => ap((p) => (p.appOrder = (v as string[]).slice())) },
    ] },
  ],
};

const agentsPage: Page = {
  id: "agents", name: "Agents", icon: "spark", blurb: "How agents behave by default. Each can be changed per thread.",
  groups: [
    { title: "New threads", settings: [
      { id: "agents.default-permission", label: "Permission mode", help: "What an agent may do without asking, for a new thread.", control: { type: "segmented", options: [{ id: "least", label: "Least access" }, { id: "approve", label: "Approve for me" }, { id: "full", label: "Full access" }] }, ...fx("agents.default-permission", "approve") },
      { id: "agents.stream-text", label: "Stream messages as written", help: "Off: a working state shows, then the finished message appears at once.", control: { type: "switch" }, get: () => A.prefs.streamText, set: (v) => ap((p) => (p.streamText = !!v)) },
    ] },
    { title: "Retries and sandboxes", settings: [
      { id: "agents.retry", label: "Retry failed turns", help: "Try again after a provider error.", control: { type: "switch" }, ...fx("agents.retry", true) },
      { id: "agents.snooze-minutes", label: "Snooze", help: "How long a snoozed question stays quiet.", control: { type: "number", min: 1, max: 1440, unit: " min" }, ...fx("agents.snooze-minutes", 30) },
      { id: "agents.sandbox-idle-minutes", label: "Sandbox idle time", help: "A thread's sandbox stops after this long without work.", control: { type: "number", min: 1, max: 240, unit: " min" }, ...fx("agents.sandbox-idle-minutes", 10) },
    ] },
    { title: "Decision service", settings: [
      { id: "agents.decision-provider", label: "Provider", help: "A fast typed-decision model, used after Enter and never while typing. Off until a key is connected.", control: { type: "popup", options: [{ id: "off", label: "Off" }, { id: "openai", label: "OpenAI Decisions" }, { id: "typesafe", label: "TypeSafe Jev" }] }, ...fx("agents.decision-provider", "off") },
    ] },
  ],
};

const keyboardPage: Page = {
  id: "keyboard", name: "Keyboard", icon: "terminal", blurb: "Shortcuts follow your account. Remap them in place.",
  groups: [
    { settings: [{ id: "keyboard.shortcuts", label: "Keyboard shortcuts", help: "Cmd-/ or ? opens a quick list anywhere.", control: { type: "custom", load: () => import("./ShortcutsPanel.svelte") } }] },
    { settings: [{ id: "keyboard.hardware-help", label: "Show the shortcut sheet with a hardware keyboard", control: { type: "switch" }, ...fx("keyboard.hardware-help", true) }] },
  ],
};

const machinesPage: Page = {
  id: "machines", name: "Machines", icon: "terminal", blurb: "The machines your threads can run on, and whether each runs them sandboxed or direct.",
  groups: [{ settings: [{ id: "machines.list", label: "Machines", control: { type: "custom", load: () => import("./MachinesPanel.svelte") } }] }],
};
const listPage = (id: string, name: string, icon: IconName, blurb: string, plugin: string, items: [string, string][], empty: string): Page => ({
  id, name, icon, blurb,
  groups: [{ title: `Your ${name.toLowerCase()}`, settings: items.length ? items.map(([k, t]): Setting => ({ id: `${plugin}.${k}.enabled`, label: t, control: { type: "switch" }, ...fx(`${plugin}.${k}.enabled`, true) })) : [{ id: `${plugin}.none`, label: empty, control: { type: "info", text: () => "" } }] }],
});
const pluginsPage: Page = {
  id: "plugins", name: "Plugins", icon: "apps", blurb: "Installed and available plugins, what each may reach, and each one's own options.",
  groups: [{ settings: [{ id: "plugins.list", label: "Plugins", control: { type: "custom", load: () => import("./PluginsPanel.svelte") } }] }],
};
// Skills and automations wait for a design round (docs/open-questions.md) and are not in Settings yet.

const accountPage: Page = {
  id: "account", name: "Account", icon: "user", blurb: "You, and what you have connected.",
  groups: [
    { title: "Profile", settings: [{ id: "account.name", label: "Name", control: { type: "text", placeholder: "Your name" }, ...fx("account.name", "Kay") }] },
    { title: "Sign-in", settings: [
      { id: "account.passkeys", label: "Passkeys", help: "How you prove it is you.", control: { type: "drill", summary: () => "2 passkeys", run: confirmLater("Passkeys") } },
      { id: "account.providers", label: "Connected provider accounts", help: "What you sign in to. The server never stores them.", control: { type: "drill", summary: () => "Claude Code, Codex", run: confirmLater("Connected provider accounts") } },
    ] },
    { settings: [{ id: "account.sign-out", label: "Sign out", control: { type: "button", text: "Sign out…", run: signOutAction } }] },
  ],
};

const danger = (id: string, label: string, help: string, text: string): Group => ({ title: "Danger area", danger: true, settings: [{ id, label, help, control: { type: "button", text, danger: true, run: confirmLater(text) } }] });
const membersPage: Page = {
  id: "members", name: "Members and roles", icon: "user", blurb: "Who is in this space and what each role may do.",
  groups: [
    { title: "Roles in this space", settings: [
      { id: "space.add-person", label: "Add a person", help: "Give a person a role in this space.", control: { type: "button", text: "Add a person…", run: confirmLater("Add a person") } },
      { id: "space.add-group", label: "Add a group", help: "A role can be given to a group as well as to a person: everyone in the group (and anyone who joins it later) has the role here. The group's own members are managed under Admin.", control: { type: "button", text: "Add a group…", run: confirmLater("Add a group") } },
    ] },
    { title: "Members", settings: [{ id: "space.members", label: "Members", help: "Roles are sets of capabilities.", control: { type: "drill", summary: () => "12 members", run: confirmLater("Members") } }, { id: "space.roles", label: "Roles", control: { type: "drill", summary: () => "Owner, Manager, Member, Reader", run: confirmLater("Roles") } }] },
    danger("space.leave", "Leave this space", "You lose access to everything in it.", "Leave space…"),
  ],
};
const invitesPage: Page = {
  id: "invites", name: "Invites", icon: "link", blurb: "Invite people with a link that expires.",
  groups: [
    { settings: [{ id: "space.invites.create", label: "Invite someone", control: { type: "button", text: "Create invite…", run: confirmLater("Create invite") } }] },
    { title: "Open invites", settings: [{ id: "space.invites.open", label: "Open invites", control: { type: "info", text: () => "None" } }] },
    danger("space.invites.revoke-all", "Revoke every open invite", "Links already sent stop working.", "Revoke all…"),
  ],
};
const secretsPage: Page = {
  id: "secrets", name: "Secrets", icon: "key", blurb: "The space's secrets, by handle. Values are never shown.",
  groups: [
    { title: "Handles", settings: [{ id: "space.secrets.forge", label: "forge-token", control: { type: "info", text: () => "Set" } }, { id: "space.secrets.registry", label: "registry-key", control: { type: "info", text: () => "Set" } }] },
    danger("space.secrets.delete-all", "Delete every secret", "Agents lose what they were given.", "Delete all…"),
  ],
};
const instructionsPage: Page = {
  id: "instructions", name: "Instructions and defaults", icon: "list", blurb: "What agents read first here (AI/AGENTS.md), and the defaults members may override.",
  groups: [
    { title: "Agents", settings: [{ id: "space.instructions", label: "Instructions", help: "The space's standing instructions, kept in AI/AGENTS.md and read by every agent in it. Only Managers and the Owner may edit it.", control: { type: "text", placeholder: "For example: run tests before you commit." }, ...fx("space.instructions", "") }] },
    { title: "Defaults", settings: [{ id: "space.default-preset", label: "Default look", help: "Members may override it.", control: { type: "popup", options: [{ id: "calternal", label: "Calternal" }, { id: "paper", label: "Paper" }, { id: "nord", label: "Nord" }] }, ...fx("space.default-preset", "calternal") }] },
    danger("space.delete", "Delete this space", "Everything in it is deleted after 30 days.", "Delete space…"),
  ],
};
const spacePluginsPage = listPage("space-plugins", "Space plugins", "apps", "Which plugins are on in this space.", "space.plugins", [["calternal-threads", "Threads"], ["calternal-notes", "Notes"]], "No plugins");
spacePluginsPage.groups.push(danger("space.plugins.disable-all", "Turn off every plugin", "Members lose those apps here.", "Turn off all…"));

const plansPage: Page = {
  id: "plans", name: "Groups and plans", icon: "user", blurb: "Groups are named after plans; each says what it allows.",
  groups: [
    { settings: [{ id: "server.groups", label: "Groups", control: { type: "drill", summary: () => "Free, Team", run: confirmLater("Groups") } }] },
    danger("server.groups.delete", "Delete a group", "Its members move to the default group.", "Delete group…"),
  ],
};
const poolPage: Page = {
  id: "pool", name: "Machine pool", icon: "terminal", blurb: "The cloud machines threads may use.",
  groups: [
    { title: "Pool", settings: [{ id: "server.pool.size", label: "Machines in the pool", control: { type: "info", text: () => "4 online" } }] },
    danger("server.pool.drain", "Drain the pool", "Running sandboxes finish; no new ones start.", "Drain…"),
  ],
};
const storagePage: Page = {
  id: "storage", name: "Storage", icon: "folder", blurb: "The three roots and their health.",
  groups: [
    { title: "Roots", settings: [
      { id: "server.storage.appdata", label: "appdata", help: "Databases and state, on the SSD.", control: { type: "info", text: () => "Healthy · 412 MB of 1.8 TB" } },
      { id: "server.storage.userdata-ssd", label: "userdata-ssd", help: "People's files, on the SSD.", control: { type: "info", text: () => "Healthy · 38 GB of 1.8 TB" } },
      { id: "server.storage.userdata-hdd", label: "userdata-hdd", help: "People's files, on disk.", control: { type: "info", text: () => "Healthy · 1.2 TB of 14 TB" } },
    ] },
    danger("server.storage.purge-trash", "Empty every trash", "Files in trash are deleted for good.", "Empty trash…"),
  ],
};

/** The Skills page of the current space: the list and the guided way to write one. */
const skillsPage = (): Page => ({ id: "skills", name: "Skills", icon: "spark", blurb: "What agents in this space know how to do: each skill is a SKILL.md file.", groups: [lazy("space.skills", "Skills", () => import("./SkillsPanel.svelte"))] });
/** The Automations page of the current space: the list and the guided way to make one. */
const automationsPage = (): Page => ({ id: "automations", name: "Automations", icon: "clock", blurb: "Prompts that run on a schedule, each run its own thread.", groups: [lazy("space.automations", "Automations", () => import("./AutomationsPanel.svelte"))] });
const lazy = (id: string, label: string, load: () => Promise<{ default: Component<any> }>, help?: string): Group => ({ settings: [{ id, label, help, control: { type: "custom", load } }] });

/** The sidebar's groups, in order (docs/ui-craft-and-settings-study.md 11.1). */
export const SECTIONS: Section[] = [
  { id: "user", name: "User", pages: [appearancePage, agentsPage, keyboardPage, machinesPage, pluginsPage, accountPage] },
  // Admin is the server (for its operator); the third section is the current space, whose name it shows (decided 2026-11-18).
  { id: "server", name: "Admin", pages: [plansPage, poolPage, storagePage] },
  { id: "space", get name() { return `Space: ${spaces.current.name}`; }, pages: [membersPage, invitesPage, secretsPage, instructionsPage, skillsPage(), automationsPage(), spacePluginsPage] },
];

const deleteSpace = (): void => {
  if (!confirm(`Delete the space "${spaces.current.name}"? Its records are removed.`)) return;
  void import("../../server/mutations").then(({ deleteSpace }) => deleteSpace(spaces.current.id)).then(() => location.replace(import.meta.env.BASE_URL), (e: Error) => alert(e.message));
};
const leaveSpace = (): void => {
  if (!confirm(`Leave the space "${spaces.current.name}"? You lose access to everything in it.`)) return;
  void import("../../server/mutations").then(({ leaveSpace }) => leaveSpace(spaces.current.id)).then(() => location.replace(import.meta.env.BASE_URL), (e: Error) => alert(e.message));
};
/**
 * The pages in server mode: only what a server operation backs. Account, Members, Secrets, Space plugins and Machines show the real data; Appearance and Keyboard are this browser's own and stay; every other
 * control (profile name, invites, groups, instructions, Agents defaults, Admin) has no operation yet and is not shown (the table in docs/ui-shell-status.md lists each with the operation it needs).
 */
function serverSections(): Section[] {
  const account: Page = { ...accountPage, groups: [lazy("account.panel", "Account", () => import("./AccountPanel.svelte"))] };
  const machines: Page = { ...machinesPage, groups: [lazy("machines.list", "Machines", () => import("./ServerMachinesPanel.svelte"))] };
  const members: Page = { ...membersPage, groups: [lazy("space.members", "Members", () => import("./MembersPanel.svelte")), { title: "Danger area", danger: true, settings: [{ id: "space.leave", label: "Leave this space", help: "You lose access to everything in it.", control: { type: "button", text: "Leave space…", danger: true, run: leaveSpace } }] }] };
  const secrets: Page = { ...secretsPage, groups: [lazy("space.secrets", "Secrets", () => import("./SecretsPanel.svelte"))] };
  const plugins: Page = { ...spacePluginsPage, groups: [lazy("space.plugins", "Plugins", () => import("./ServerPluginsPanel.svelte"))] };
  const manage: Page = { id: "manage", name: "Delete this space", icon: "folder", blurb: "Removes the space for everyone in it.", groups: [{ title: "Danger area", danger: true, settings: [{ id: "space.delete", label: "Delete this space", help: "Only the owner may.", control: { type: "button", text: "Delete space…", danger: true, run: deleteSpace } }] }] };
  const roles: Page = { id: "roles", name: "Roles", icon: "list", blurb: "What each role may do in a space.", groups: [lazy("space.roles", "Roles", () => import("./RolesPanel.svelte"))] };
  const invites: Page = { ...invitesPage, groups: [lazy("space.invites", "Invitations", () => import("./InvitesPanel.svelte"))] };
  const mayManage = spaces.current.role === "owner" || spaces.current.role === "manager";
  const admin: Page[] = [
    ...(caps.peopleManage ? [{ id: "people", name: "People", icon: "user" as IconName, blurb: "Everyone with an account on this server.", groups: [lazy("server.people", "People", () => import("./PeoplePanel.svelte"))] }] : []),
    ...(caps.groupsManage ? [{ id: "groups", name: "Groups", icon: "user" as IconName, blurb: "Groups give a role in a space to everyone in them.", groups: [lazy("server.groups", "Groups", () => import("./GroupsPanel.svelte"))] }] : []),
  ];
  return [
    { id: "user", name: "User", pages: [appearancePage, keyboardPage, machines, account] },
    ...(admin.length ? [{ id: "server" as const, name: "Admin", pages: admin }] : []),
    { id: "space", get name() { return `Space: ${spaces.current.name}`; }, pages: [members, roles, ...(mayManage ? [invites] : []), secrets, skillsPage(), automationsPage(), plugins, manage] },
  ];
}

/** The sections this person sees: Admin (the server) only with the server-admin capability (the real one in server mode). */
export const visibleSections = (): Section[] => (serverMode() ? serverSections() : SECTIONS.filter((s) => s.id !== "server" || caps.serverAdmin));
/** The page with this id among the pages this person can see, or undefined (the address is then shown as the Settings start, not as an error). Evaluates `visibleSections`, so it follows the source, the capabilities and the current space. */
export const pageById = (id: string | undefined): Page | undefined => visibleSections().flatMap((s) => s.pages).find((p) => p.id === id);
/** Every setting with the page it lives on. */
export const allSettings = (): { page: Page; setting: Setting }[] => visibleSections().flatMap((s) => s.pages.flatMap((p) => p.groups.flatMap((g) => g.settings.map((setting) => ({ page: p, setting })))));

// The palette finds every setting by its label, help and section, and opens its address (decided: Settings has no search box of its own).
registerManifest({
  plugin: "calternal-settings",
  commands: [
    ...visibleSections().flatMap((s) => s.pages.map((p) => ({ id: `settings.${p.id}`, label: p.name, group: "Settings", go: `/settings/${p.id}`, words: [p.blurb] }))),
    ...allSettings().filter((x) => x.setting.control.type !== "info").map(({ page, setting }) => ({ id: setting.id, label: setting.label, group: `Settings, ${page.name}`, go: `/settings/${page.id}/${setting.id}`, words: [setting.help ?? ""] })),
    { id: "settings.shortcuts", label: "Keyboard shortcuts", group: "Settings", op: "help.shortcuts", words: ["keys", "hotkeys"] },
  ],
});
