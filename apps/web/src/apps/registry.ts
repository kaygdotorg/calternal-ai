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
import Notes from "./notes/Notes.svelte";
import NotesPanels from "./notes/Panels.svelte";
import NotesSidebar from "./notes/Sidebar.svelte";
import { notes } from "./notes/store.svelte";
import SettingsMain from "./settings/Main.svelte";
import SettingsSidebar from "./settings/Sidebar.svelte";

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
  name: string;
  icon: IconName;
  Sidebar: Component<any>;
  Main: Component<any>;
  panels: PanelDef[];
  /** Reactive: read inside markup so the card follows the data. */
  attention?: () => AttentionItem[];
  /** One-time loading. */
  start?: () => Promise<void>;
}

/** The installed apps, in switcher order. */
export const APPS: AppDef[] = [
  {
    kind: "threads",
    name: "Threads",
    icon: "threads",
    Sidebar: ThreadsSidebar,
    Main: ThreadsMain,
    panels: [
      { id: "changes", label: "Changes", icon: "changes", component: ThreadsPanels, props: { kind: "changes" } },
      { id: "run", label: "Run", icon: "pulse", component: ThreadsPanels, props: { kind: "run" } },
      { id: "files", label: "Files", icon: "files", component: ThreadsPanels, props: { kind: "files" } },
      { id: "chat", label: "Side chat", tab: "Chat", icon: "threads", component: SideChat },
    ],
    attention: () =>
      threads.attention.map(({ thread, kinds }) => ({
        id: thread.id,
        icon: kinds.includes("approval") ? "shield" : kinds.includes("secret") ? "key" : "question",
        title: thread.title,
        sub: kinds.map((k) => ({ question: "Question", approval: "Approval", secret: "Secret" })[k]).filter((v, i, a) => a.indexOf(v) === i).join(" and ").toLowerCase().replace(/^./, (c) => c.toUpperCase()),
        href: `/threads/${thread.id}`,
      })),
    start: () => threads.start(),
  },
  {
    kind: "notes",
    name: "Notes",
    icon: "notes",
    Sidebar: NotesSidebar,
    Main: Notes,
    panels: [
      { id: "inspector", label: "Inspector", icon: "info", component: NotesPanels, props: { kind: "inspector" } },
      { id: "style", label: "Style", icon: "settings", component: NotesPanels, props: { kind: "style" } },
    ],
    start: () => notes.start(),
  },
];

/** Settings is an app like the others: sections in the sidebar, the page in the middle card, no right panels. */
APPS.push({ kind: "settings", name: "Settings", icon: "settings", Sidebar: SettingsSidebar, Main: SettingsMain, panels: [] });

/** The app that opens an address kind, or undefined for a kind no installed app handles. */
export const appFor = (kind: string | undefined): AppDef | undefined => APPS.find((a) => a.kind === kind);
