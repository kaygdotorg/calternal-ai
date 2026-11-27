/** Fixture data of the mockup: the apps, the collections and the sidebars they show. Invented people and things; nothing here is real or saved. */
export interface AppDef { id: string; name: string; /** A name from packages/ui's icon set, or one of the mockup's own (`mail`, `calendar`). */ icon: string }
export const APPS: AppDef[] = [
  { id: "chat", name: "Chat", icon: "chat" }, { id: "notes", name: "Notes", icon: "notes" }, { id: "mail", name: "Mail", icon: "mail" }, { id: "files", name: "Files", icon: "files" },
  { id: "photos", name: "Photos", icon: "image" }, { id: "calendar", name: "Calendar", icon: "calendar" }, { id: "contacts", name: "Contacts", icon: "user" }, { id: "tasks", name: "Tasks", icon: "tasks" },
];
/** Fixture history for frecency: for each app the days ago of its recent opens. An app opened often and lately ranks first; the weights are in `frecency`. */
export const VISITS: Record<string, number[]> = {
  notes: [0, 0, 0, 1, 1, 2, 3, 5, 8, 12, 20], mail: [0, 0, 1, 1, 1, 2, 4, 6, 9], chat: [0, 1, 1, 2, 3, 4, 9, 14], calendar: [0, 2, 3, 6, 7, 11, 18], files: [1, 4, 9, 15, 24], tasks: [2, 3, 12, 27], photos: [6, 21, 40], contacts: [30, 45],
};
/** A frecency score: opens count for more the more recent they are (today and yesterday 100, this week 70, this month 40, older 10). */
export const frecency = (id: string, extra = 0): number => (VISITS[id] ?? []).reduce((s, d) => s + (d <= 1 ? 100 : d <= 7 ? 70 : d <= 30 ? 40 : 10), 0) + extra * 100;
export const SETTINGS: AppDef = { id: "settings", name: "Settings", icon: "settings" };
/** Spaces are one kind. The first is Personal: yours alone, it takes no members. Anyone creates more, names them and invites group members. */
export const SPACES = [{ id: "s0", name: "Personal", members: 0 }, { id: "s1", name: "Lindqvist household", members: 4 }, { id: "s2", name: "Studio", members: 3 }, { id: "s3", name: "Book club", members: 6 }];
/** The space open when the mockup starts. */
export const FIRST_SPACE = "s1";
/** People of the fixture who can be invited to a new space (they are in the groups the person belongs to). */
export const GROUPS = [{ id: "g1", name: "Family", people: ["Hana Sato", "Ivo Novak", "Jun Park"] }, { id: "g2", name: "Studio team", people: ["Kim Aalto", "Ivo Novak"] }];
export const ME = { name: "Ada Lindqvist" };
