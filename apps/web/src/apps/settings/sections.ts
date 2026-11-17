/**
 * The Settings sections, from docs/settings.md: two groups, User (yours, follows you) and Admin (the space, and the server for its operator).
 * The list is data so the real plugin can supply it; the pages are not built yet (a study lane measures Apple's settings windows first, issue #22).
 */
import type { IconName } from "@calternal/ui";

/** One settings page. */
export interface SettingsSection { id: string; name: string; icon: IconName; blurb: string }
/** A labelled group of pages. */
export interface SettingsGroup { id: "user" | "admin"; name: string; sections: SettingsSection[] }

export const GROUPS: SettingsGroup[] = [
  {
    id: "user",
    name: "User",
    sections: [
      { id: "appearance", name: "Appearance", icon: "image", blurb: "Wallpaper, presets, materials, fonts, UI size, motion, accent and the wordmark." },
      { id: "agents", name: "Agents", icon: "spark", blurb: "Default permission mode, retry and snooze, streaming, sandbox idle time, the decision service." },
    ],
  },
  {
    id: "admin",
    name: "Admin",
    sections: [
      { id: "space", name: "Space", icon: "folder", blurb: "Defaults members may override, groups and roles, invites." },
      { id: "server", name: "Server", icon: "terminal", blurb: "For the operator of this installation." },
    ],
  },
];

/** A section by id. */
export const sectionById = (id: string | undefined): SettingsSection | undefined => GROUPS.flatMap((g) => g.sections).find((s) => s.id === id);
