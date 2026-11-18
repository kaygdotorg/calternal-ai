/**
 * The open note's live room, when live editing is on (`?live=1`, kept in this browser; `?live=0` turns it off). Off, nothing here loads and a note is saved over HTTP as before. On, the editor package's live pieces load
 * lazily (so the first load carries none of Yjs), the note opens as a `NoteSession` on a room (the live socket's, or the fixture room's), and this store holds what the header shows: who else is here, and the save state.
 */
import { serverMode } from "../../../server/client";
import { account } from "../../../server/auth";
import { grants } from "../../../server/people";
import { sharing } from "../share/sharing";
import { textKey } from "../share/rescue";
import { rescueBlocks, settleOwed } from "../share/owed";
import { notes } from "../store.svelte";
import type { NoteSession } from "../../../../../../packages/editor/src/note";
import type { Person } from "../../../../../../packages/editor/src/live/extension";

/** Whether live editing is on: asked for with `?live=1`, kept for next time, turned off with `?live=0`. */
export function liveOn(): boolean {
  try {
    const q = new URLSearchParams(location.search).get("live");
    if (q === "1" || q === "0") localStorage.setItem("cal.live", q);
    return localStorage.getItem("cal.live") === "1";
  } catch { return false; }
}

/** Someone else in the room: the account the room stamped on their presence, and how to draw them. */
export interface Here { client: number; account: string; name: string; color: string }

const FIXTURE_NAMES: Record<string, string> = { "acc-maya": "Maya Okafor", "acc-theo": "Theo Lindqvist", "acc-ines": "Inés Duarte", "acc-sam": "Sam Whitlock" };
const hue = (s: string) => { let h = 0; for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360; return h; };
/** A person's colour from their account: the same everywhere, in every tab. */
export const colourOf = (account: string): { color: string; colorLight: string } => ({ color: `hsl(${hue(account)} 62% 42%)`, colorLight: `hsl(${hue(account)} 62% 42% / 0.25)` });

/** The one live socket of this page (a window per note shares it). */
let pageSocket: import("../../../../../../packages/editor/src/sync/live").LiveSocket | null = null;

/** The one line about a rescue (where a person's edits went, or that they are kept), shown in whichever note is open. */
export const liveNotice = $state({ text: "" });

/** One note's live room. Several can be open at once (a window in the canvas each); they share the page's socket. */
class LiveNotes {
  constructor(private readonly forget: () => void) {}
  session = $state.raw<NoteSession | null>(null);
  /** The editor extension for the open note (bumped by `epoch` when its document is replaced). */
  binding = $state.raw<{ extension: import("@codemirror/state").Extension } | null>(null);
  epoch = $state(0);
  here = $state.raw<Here[]>([]);
  /** The save line ("Saved", "Not saved yet · 3 min", …). */
  save = $state("");
  private names = new Map<string, string>(Object.entries(FIXTURE_NAMES));
  private stops: (() => void)[] = [];
  private bindStops: (() => void)[] = [];
  /** What this person wrote that the room has not confirmed (never other people's words): the only thing a rescue may take. */
  private intent: { blocks(): string[]; clear(): void } | null = null;
  private id = "";

  private resolve = (account: string): Person => ({ name: this.names.get(account) ?? "Someone", ...colourOf(account) });

  /** Opens the note's room. `text` seeds the fixture room only (the real server seeds from the file). */
  async open(spaceId: string, noteId: string, text: string): Promise<void> {
    this.close();
    const id = (this.id = noteId);
    const live = await import("../../../../../../packages/editor/src/live/index");
    const { Awareness } = await import("y-protocols/awareness");
    const fixture = serverMode() ? null : await import("./fixture");
    if (id !== this.id) return;
    if (serverMode()) void this.loadNames(spaceId);
    const me = serverMode() ? (await account.me().catch(() => null))?.user.id ?? "" : "acc-you";
    const session = await live.NoteSession.open({
      id: noteId,
      offline: true, // a copy of the note is kept on this device (IndexedDB): edits made with the room unreachable, or before a reload, are replayed to the room when it is back
      awareness: (doc) => new Awareness(doc),
      onAccessRevoked: () => void this.revoked(noteId),
      transport: (lineage, onLineage) => {
        if (fixture) return fixture.fixtureTransport(noteId, text, lineage, onLineage);
        pageSocket ??= new live.LiveSocket(() => { const u = new URL("/api/v1/live", location.href); u.protocol = u.protocol === "https:" ? "wss:" : "ws:"; return u.href; }, "calternal-live-rooms.v1");
        return pageSocket.room(`${spaceId}/notes/${noteId}`, { lineage, onLineage });
      },
    }).catch(() => null);
    if (!session || id !== this.id) { session?.destroy(); return; }
    this.attach(session, me, live);
  }

  private attach(session: NoteSession, me: string, live: typeof import("../../../../../../packages/editor/src/live/index")): void {
    this.session = session;
    const bind = () => {
      session.awareness?.setLocalState({ account_id: me });
      const made = live.liveExtension(session, this.resolve);
      this.intent = made.intent;
      this.binding = { extension: made.extension };
      this.epoch++;
      const a = session.awareness;
      if (!a) return;
      const read = () => { this.here = [...a.getStates()].filter(([c, s]) => c !== a.doc.clientID && typeof s.account_id === "string").map(([client, s]) => ({ client, account: s.account_id as string, name: this.resolve(s.account_id as string).name, color: colourOf(s.account_id as string).color })); };
      a.on("change", read); read();
      this.bindStops.push(() => a.off("change", read));
    };
    // The editor is drawn once the room has answered (so its first paint has the text), or after a moment if it cannot (offline: the copy kept on this device).
    let bound = false;
    const go = () => { if (!bound && this.session === session) { bound = true; bind(); } };
    this.stops.push(session.onSynced(go));
    const wait = setTimeout(go, 2500);
    this.stops.push(() => clearTimeout(wait));
    session.connect();
    this.stops.push(session.onReplaced(() => { this.bindStops.splice(0).forEach((f) => f()); bind(); }));
    this.stops.push(session.status.subscribe((s) => { this.save = live.describeStatus(s); if (s.state === "saved") this.intent?.clear(); }));
  }

  /**
   * The room ended because access was taken away: the session is frozen, and only the blocks this person wrote themselves since the last receipt (their `LocalIntent`, never a diff of the shared text) go to `sharing.rescue` (a note in their personal space, "Your edits to <title>"), with a stable
   * request id for the same text. One line says where they went; if the rescue fails the line says so and the text is still here.
   */
  private async revoked(noteId: string): Promise<void> {
    const session = this.session;
    if (!session || noteId !== this.id) return;
    const blocks = this.intent?.blocks() ?? [];
    liveNotice.text = await rescueBlocks(noteId, notes.note?.title ?? "this note", blocks, `${noteId}:${textKey(blocks.join("\n"))}`);
  }

  private async loadNames(spaceId: string): Promise<void> {
    try { for (const m of (await grants(spaceId)).members as unknown as { account_id?: string; id?: string; name?: string }[]) { const id = m.account_id ?? m.id; if (id && m.name) this.names.set(id, m.name); } this.here = this.here.map((h) => ({ ...h, name: this.resolve(h.account).name })); } catch { /* names stay "Someone" */ }
  }

  close(): void {
    this.id = "";
    this.stops.splice(0).forEach((f) => f());
    this.bindStops.splice(0).forEach((f) => f());
    this.session?.destroy();
    this.session = null; this.binding = null; this.here = []; this.save = ""; this.intent = null;
    this.forget();
  }
}

const rooms = new Map<string, LiveNotes>();
/** The live room of a note (made on first ask; `close()` on it removes it). */
export function liveOf(noteId: string): LiveNotes {
  let r = rooms.get(noteId);
  if (!r) rooms.set(noteId, (r = new LiveNotes(() => rooms.delete(noteId))));
  return r;
}
/** Sends every rescue still owed (share/owed.ts) and shows the line for each one that went through; no room needed. */
export async function settleOwedNow(): Promise<void> {
  const lines = await settleOwed().catch(() => []);
  if (lines.length) liveNotice.text = lines.at(-1)!;
}
