/**
 * The plugin install sheet's three mock plugins (fixture only). Each states, in plain words and in one fixed order, what it stores (each kind, marked as editable files or changed only through the app), where it sends data, which sign-ins it uses (the server holds them), which programs it runs and whether it ships web code.
 */
export interface Stores { what: string; where?: string; editable: boolean }
export interface PluginInfo {
  id: string; name: string; by: string; blurb: string;
  /** An official plugin carries the mark. */
  official: boolean;
  stores: Stores[];
  /** Destinations data is sent to; empty means nowhere. */
  sends: { host: string; why: string }[];
  /** Sign-ins it uses (the server holds each; the plugin never sees one). */
  signins: string[];
  /** Programs it runs; they run in the background and count against the account's hours. */
  programs: string[];
  /** Ships web code that runs in the person's browser. */
  webCode: boolean;
}
export const PLUGINS: PluginInfo[] = [
  { id: "calternal-notes", name: "calternal-notes", by: "Calternal", official: true, blurb: "Notes written as plain Markdown files.",
    stores: [{ what: "Notes, as Markdown files", where: "Notes/", editable: true }, { what: "A search index of your notes", editable: false }],
    sends: [], signins: [], programs: [], webCode: false },
  { id: "calternal-mail", name: "calternal-mail", by: "Calternal", official: true, blurb: "Read, write and file your mail.",
    stores: [{ what: "Messages and drafts, kept as operations", editable: false }, { what: "A search index of your mail", editable: false }],
    sends: [{ host: "imap.example.org", why: "your mail server" }], signins: ["Your mail account"], programs: ["Mail sync"], webCode: false },
  { id: "acme-board", name: "acme-board", by: "Acme Studio (third party)", official: false, blurb: "Kanban boards for any collection.",
    stores: [{ what: "Boards, as files", where: "Boards/", editable: true }, { what: "Cached card thumbnails", editable: false }],
    sends: [{ host: "api.acme-board.io", why: "syncs boards you share" }, { host: "stats.acme-board.io", why: "usage statistics" }], signins: [], programs: [], webCode: true },
];
