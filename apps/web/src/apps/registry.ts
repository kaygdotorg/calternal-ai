/**
 * What an app (a plugin) fills in the shell's three regions (docs/ui-shell.md). The shell owns the chrome; an app supplies the
 * sidebar body, the middle card, the right card's panels and what needs attention. Two apps exist now (Threads, Notes), both in
 * this repository; the real plugin API will describe the same shape in its manifest.
 */
import type { Component } from "svelte";
import type { IconName } from "@calternal/ui";
import ThreadsMain from "./threads/Main.svelte";
import ThreadsPanels from "./threads/Panels.svelte";
import SideChat from "./threads/SideChat.svelte";
import ThreadsSidebar from "./threads/Sidebar.svelte";
import { store as threads } from "./threads/store.svelte";
// Notes and Settings are loaded when first opened: the wrappers draw nothing but the import, and the Notes store (with its fixtures) comes in with `start`.
import Notes from "./notes/NotesMain.svelte";
import NotesPanels from "./notes/NotesPanels.svelte";
import NotesSidebar from "./notes/NotesSidebar.svelte";
import MailMain from "./mail/MailMain.svelte";
import MailSidebar from "./mail/MailSidebar.svelte";
import "./settings/registry"; // registers a palette command for every setting: the palette finds settings before the Settings app has ever been opened
import SettingsMain from "./settings/SettingsMain.svelte";
import SettingsSidebar from "./settings/SettingsSidebar.svelte";

/** One thing that needs the person. The shell shows at most three. */
export interface AttentionItem {
  id: string;
  icon: IconName;
  title: string;
  sub: string;
  href: string;
}

/** A tab in the right card. */
export interface PanelDef {
  id: string;
  label: string;
  /** A shorter name for the tab when four tabs must fit the narrow sidebar (the title keeps `label`). */
  tab?: string;
  icon: IconName;
  component: Component<any>;
  props?: Record<string, unknown>;
}

/** An app. */
export interface AppDef {
  /** The kind in addresses (`/threads/<id>`). */
  kind: string;
  /** Other kinds whose addresses this app answers (`/collections/<id>` is a Notes page). */
  aliases?: string[];
  name: string;
  icon: IconName;
  Sidebar: Component<any>;
  Main: Component<any>;
  panels: PanelDef[];
  /** Reactive: read inside markup so the card follows the data. */
  attention?: () => AttentionItem[];
  /** One-time loading, run when the app is first opened (Threads at start-up). */
  start?: () => Promise<void>;
  /** Reloads what the sidebar lists (pull to refresh on a phone). Absent when there is nothing to reload. */
  refresh?: () => Promise<unknown>;
}

/** The installed apps, in switcher order. */
export const APPS: AppDef[] = [
  {
    kind: "threads",
    name: "Threads",
    icon: "threads",
    Sidebar: ThreadsSidebar,
    Main: ThreadsMain,
    // AI chat is the right sidebar's default tab in every app (docs/ui-shell.md); the app's own panels follow it.
    panels: [
      { id: "chat", label: "Side chat", tab: "Chat", icon: "threads", component: SideChat, props: { about: "thread" } },
      { id: "changes", label: "Changes", icon: "changes", component: ThreadsPanels, props: { kind: "changes" } },
      { id: "run", label: "Run", icon: "pulse", component: ThreadsPanels, props: { kind: "run" } },
      { id: "files", label: "Files", icon: "files", component: ThreadsPanels, props: { kind: "files" } },
    ],
    start: () => threads.start(),
    refresh: () => threads.refresh(),
  },
  {
    kind: "notes",
    aliases: ["collections"],
    name: "Notes",
    icon: "notes",
    Sidebar: NotesSidebar,
    Main: Notes,
    panels: [
      { id: "chat", label: "Side chat", tab: "Chat", icon: "threads", component: SideChat, props: { about: "note" } },
      { id: "inspector", label: "Inspector", icon: "info", component: NotesPanels, props: { kind: "inspector" } },
      { id: "style", label: "Style", icon: "settings", component: NotesPanels, props: { kind: "style" } },
    ],
    start: () => import("./notes/store.svelte").then((m) => m.notes.start()),
    refresh: () => import("./notes/store.svelte").then((m) => m.notes.refresh()),
  },
];

/** Mail: folders and headers in the sidebar, the open message in the middle card, both drawn from the Mail plugin's GenUI views. Side chat only, until Mail has panels of its own. */
APPS.push({
  kind: "mail",
  name: "Mail",
  icon: "mail",
  Sidebar: MailSidebar,
  Main: MailMain,
  panels: [{ id: "chat", label: "Side chat", tab: "Chat", icon: "threads", component: SideChat, props: { about: "mail" } }],
  start: () => import("./mail/store.svelte").then((m) => m.mail.start()),
  refresh: () => import("./mail/store.svelte").then((m) => m.mail.refresh()),
});

/** Settings is an app like the others: sections in the sidebar, the page in the middle card, no right panels. */
APPS.push({ kind: "settings", name: "Settings", icon: "settings", Sidebar: SettingsSidebar, Main: SettingsMain, panels: [] });

/** The app that opens an address kind, or undefined for a kind no installed app handles. */
export const appFor = (kind: string | undefined): AppDef | undefined => APPS.find((a) => a.kind === kind || a.aliases?.includes(kind ?? ""));
