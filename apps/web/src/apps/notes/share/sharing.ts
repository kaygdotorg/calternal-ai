/**
 * What the Share sheet, the "Shared with me" list and the guest page need from a backend: the `sharing.*` operations (issue #60), with exactly their shapes. Two implementations: fixtures (the default, in memory) and,
 * when #60 lands, the real server behind the same interface. The screens only see `SharingClient`.
 *
 * Shapes confirmed by the sharing lane: named grant rows are `{grant_id, permission, audience, name}`; link rows are `{grant_id, permission: "view", label, expires_at}`; `space_access` is "view", "edit" or null. Still open: the values
 * of the caller's own `permission`.
 */
import { ServerError, serverMode } from "../../../server/client";
import { RealSharing } from "./real";

/** What a share points at: a note, or a collection (a View). */
export interface ItemRef { kind: "notes" | "collections"; id: string }
/** Who a grant is for: one account or one group. A link has no audience. */
export type Audience = { account_id: string } | { group_id: string };
/** What an audience may do. Links are view-only in this slice. */
export type Permission = "view" | "edit";
/** A person's or group's grant; `name` is their display name. */
export interface NamedGrant { grant_id: string; permission: Permission; audience: Audience; name: string }
/** A link's grant: view-only, with a name (`label`, which may repeat) and an expiry in Unix milliseconds (null = Never). Its address carries a secret and is asked for with `link_copy`, never listed. */
export interface LinkGrant { grant_id: string; permission: "view"; label: string; expires_at: number | null }
/** One grant on an item. */
export type Grant = NamedGrant | LinkGrant;
/** Whether a grant is a link. */
export const isLink = (g: Grant): g is LinkGrant => "label" in g;

/** One page of `sharing.list`: who the item is shared with, what the caller may manage, and what everyone in the space has. */
export interface ShareList {
  /** The caller's own standing on the item. */
  permission: Permission | "owner";
  can_manage_audiences: boolean;
  can_manage_links: boolean;
  /** What every member of the item's space can do with it, or null when the space has none. */
  space_access: Permission | null;
  grants: Grant[];
  next_cursor?: string;
}
/** A person or group the caller can add. */
export interface Found { audience: Audience; name: string }
/** One row of "Shared with me": the item, its title, the sharer's display name (never an email), the caller's permission and whether it changed since the caller last opened it. */
export interface Incoming { item: ItemRef; title: string; shared_by: string; permission: Permission; unread: boolean; /** On a collection: the caller may change its notes. */ can_edit?: boolean }

/** What `GET /s/<secret>` answers with `Accept: application/json` (no sign-in; a wrong, expired, revoked or trashed target is HTTP 404, here `not_found`): a note, a collection's members (each `href` is another such address), or a member (`back` leads to its collection). `item` is present only for a valid session whose account can open the item without the link (anonymous answers omit it): "Open in Calternal" shows only then. A member's `href` is a link-scoped position, opaque, never an item id. */
export type Guest =
  | { title: string; markdown: string; back?: string; item?: ItemRef }
  | { title: string; members: { title: string; href: string }[]; item?: ItemRef };

/** The `sharing.*` operations. Failures are `ServerError` (`forbidden` when the caller may not manage that part, `not_found` for a grant that is gone or a link that expired). */
export interface SharingClient {
  list(item: ItemRef, cursor?: string): Promise<ShareList>;
  /** Accounts and groups whose name starts with `query`, not yet granted on the item. */
  audiences(item: ItemRef, query: string, cursor?: string): Promise<{ audiences: Found[]; next_cursor?: string }>;
  grant(item: ItemRef, audience: Audience, permission: Permission): Promise<NamedGrant>;
  /** Needs a non-blank label (the sheet supplies a default); labels may repeat. */
  link_create(item: ItemRef, label: string, expires_at?: number | null): Promise<LinkGrant>;
  link_update(item: ItemRef, grant_id: string, label?: string, expires_at?: number | null): Promise<LinkGrant>;
  /** The link's address, which carries its secret: asked for when copied or drawn as a code, never listed. */
  link_copy(item: ItemRef, grant_id: string): Promise<{ url: string }>;
  revoke(item: ItemRef, grant_id: string): Promise<void>;
  shared_with_me(kind: ItemRef["kind"], cursor?: string): Promise<{ items: Incoming[]; next_cursor?: string }>;
  set_link_policy(space_id: string, editors_can_manage_links: boolean): Promise<void>;
  /** After a write was refused because access ended: saves only the locally changed blocks, with the cached title, as a note in the person's personal space ("Your edits to <title>"). `request_id` makes a retry the same request. */
  rescue(title: string, changed_blocks: string[], request_id: string): Promise<{ note: { id: string; title: string } }>;
  /** Clears the item's unread mark in "Shared with me": called when the person opens it. */
  mark_seen(item: ItemRef): Promise<{ seen: true }>;
  /** Reads a link's address (`/s/<secret>` or a member's `href`) without signing in: just that item, read-only. */
  guest(path: string): Promise<Guest>;
}

/** The same key for an audience, in a list or a map. */
export const audienceKey = (a: Audience): string => ("account_id" in a ? `a:${a.account_id}` : `g:${a.group_id}`);
/** Whether an audience is a group. */
export const isGroup = (a: Audience | undefined): boolean => !!a && "group_id" in a;

/** One plain sentence on who can see the item, from what the list says: named people and groups by what they can do, then links, then the space. */
export function sentenceOf(list: Pick<ShareList, "grants" | "space_access">): string {
  const join = (a: string[]) => (a.length <= 1 ? (a[0] ?? "") : `${a.slice(0, -1).join(", ")} and ${a.at(-1)}`);
  const named = list.grants.filter((g): g is NamedGrant => !isLink(g));
  const names = (p: Permission) => named.filter((g) => g.permission === p).map((g) => g.name);
  const parts: string[] = [];
  if (list.space_access) parts.push(`Everyone in this space can ${list.space_access}`);
  if (names("edit").length) parts.push(`${join(names("edit"))} can edit`);
  if (names("view").length) parts.push(`${join(names("view"))} can view`);
  if (list.grants.some(isLink)) parts.push("Anyone with a link can view");
  return parts.length ? `${parts.join(". ")}.` : "Only you can see this.";
}

/** The expiry choices a link row offers, in days (null = Never, the default). */
export const EXPIRIES: { days: number | null; label: string }[] = [{ days: null, label: "Never" }, { days: 1, label: "1 day" }, { days: 7, label: "7 days" }, { days: 30, label: "30 days" }];
/** The expiry as a short word: Never, or the day it ends on ("Ends 24 Nov"), or Expired. */
export function expiryText(at: number | null, now = Date.now()): string {
  if (at === null) return "Never";
  if (at <= now) return "Expired";
  return `Ends ${new Date(at).toLocaleDateString("en-GB", { day: "numeric", month: "short", timeZone: "UTC" })}`;
}

const PEOPLE: Found[] = [
  { audience: { account_id: "acc-maya" }, name: "Maya Okafor" },
  { audience: { account_id: "acc-theo" }, name: "Theo Lindqvist" },
  { audience: { account_id: "acc-ines" }, name: "Inés Duarte" },
  { audience: { account_id: "acc-sam" }, name: "Sam Whitlock" },
  { audience: { group_id: "grp-design" }, name: "Design" },
  { audience: { group_id: "grp-infra" }, name: "Infrastructure" },
];

/**
 * The in-memory fixture backend. `?share=` picks the caller's standing for what a test or a screenshot needs: `owner` (default), `editor` (people and groups, no links), `viewer` (nothing), `manager` (everything). Links open at
 * `/s/<secret>`, which the guest page's fixture resolves.
 */
export class FixtureSharing implements SharingClient {
  private grants = new Map<string, Grant[]>();
  private n = 0;
  private as = new URLSearchParams(location.search).get("share") ?? "owner";
  private key = (i: ItemRef) => `${i.kind}:${i.id}`;
  private of = (i: ItemRef) => this.grants.get(this.key(i)) ?? [];
  private secrets = new Map<string, string>();
  private incoming: Incoming[] = [
    { item: { kind: "notes", id: "n-4" }, title: "Release checklist", shared_by: "Maya Okafor", permission: "edit", unread: true },
    { item: { kind: "notes", id: "n-3" }, title: "Invite flow", shared_by: "Theo Lindqvist", permission: "view", unread: false },
    { item: { kind: "collections", id: "c-demo" }, title: "Launch notes", shared_by: "Maya Okafor", permission: "edit", unread: true, can_edit: true },
  ];
  async list(item: ItemRef): Promise<ShareList> {
    const as = this.as;
    return { permission: as === "viewer" ? "view" : as === "editor" ? "edit" : "owner", can_manage_audiences: as !== "viewer", can_manage_links: as === "owner" || as === "manager", space_access: null, grants: this.of(item) };
  }
  async audiences(item: ItemRef, query: string) {
    const have = new Set(this.of(item).filter((g): g is NamedGrant => !isLink(g)).map((g) => audienceKey(g.audience)));
    const q = query.trim().toLowerCase();
    return { audiences: PEOPLE.filter((p) => !have.has(audienceKey(p.audience)) && p.name.toLowerCase().includes(q)) };
  }
  async grant(item: ItemRef, audience: Audience, permission: Permission): Promise<NamedGrant> {
    const list = this.of(item).slice();
    const at = list.findIndex((g) => !isLink(g) && audienceKey(g.audience) === audienceKey(audience));
    const name = PEOPLE.find((p) => audienceKey(p.audience) === audienceKey(audience))?.name ?? "Someone";
    const g: NamedGrant = { grant_id: at >= 0 ? list[at]!.grant_id : `g-${++this.n}`, audience, name, permission };
    if (at >= 0) list[at] = g; else list.push(g);
    this.grants.set(this.key(item), list);
    return g;
  }
  async link_create(item: ItemRef, label: string, expires_at: number | null = null): Promise<LinkGrant> {
    if (!label.trim()) throw new ServerError("invalid_input", "a link needs a name");
    const g: LinkGrant = { grant_id: `l-${++this.n}`, label, permission: "view", expires_at };
    this.secrets.set(g.grant_id, `fx${this.n}k9q2m7x4tz`);
    this.grants.set(this.key(item), [...this.of(item), g]);
    return g;
  }
  async link_update(item: ItemRef, grant_id: string, label?: string, expires_at?: number | null): Promise<LinkGrant> {
    const list = this.of(item).map((g) => (g.grant_id === grant_id && isLink(g) ? { ...g, ...(label !== undefined ? { label } : {}), ...(expires_at !== undefined ? { expires_at } : {}) } : g));
    const g = list.find((x) => x.grant_id === grant_id);
    if (!g || !isLink(g)) throw new ServerError("not_found", "that link is gone");
    this.grants.set(this.key(item), list);
    return g;
  }
  async link_copy(_item: ItemRef, grant_id: string): Promise<{ url: string }> {
    const s = this.secrets.get(grant_id);
    if (!s) throw new ServerError("not_found", "that link is gone");
    return { url: `${location.origin}/s/${s}` };
  }
  async revoke(item: ItemRef, grant_id: string): Promise<void> { this.grants.set(this.key(item), this.of(item).filter((g) => g.grant_id !== grant_id)); }
  async shared_with_me(kind: ItemRef["kind"]) { return { items: this.incoming.filter((i) => i.item.kind === kind) }; }
  async set_link_policy(): Promise<void> {}
  /** `?test=1`: `localStorage["cal.fixture.rescue-down"]` makes the rescue fail as an unreachable server would, and `cal.fixture.rescued` lists the notes made (once per request id) for the e2e. */
  async rescue(title: string, blocks: string[], request_id: string) {
    if (localStorage.getItem("cal.fixture.rescue-down")) throw new ServerError("unavailable", "can't reach the server");
    const made: { request_id: string; title: string; blocks?: string[] }[] = JSON.parse(localStorage.getItem("cal.fixture.rescued") ?? "[]");
    if (!made.some((m) => m.request_id === request_id)) localStorage.setItem("cal.fixture.rescued", JSON.stringify([...made, { request_id, title: `Your edits to ${title}`, blocks }]));
    return { note: { id: `rescue-${request_id}`, title: `Your edits to ${title}` } };
  }
  async mark_seen(item: ItemRef) { this.incoming = this.incoming.map((i) => (i.item.id === item.id ? { ...i, unread: false } : i)); return { seen: true as const }; }
  async guest(path: string): Promise<Guest> {
    const g = await this.guestAnon(path);
    return new URLSearchParams(location.search).has("signedin") ? g : { ...g, item: undefined }; // only a signed-in account is told which item it is
  }
  private async guestAnon(path: string): Promise<Guest> {
    // The fixtures' secrets open a note; "/s/demo-set" opens a collection, whose members open as "/s/demo-set/1".
    const m = /^\/s\/([^/]+)(?:\/(\d+))?\/?$/.exec(path);
    const secret = m?.[1] ?? "";
    if (secret === "demo-set") {
      if (m?.[2]) return { title: "Release checklist", markdown: "# Release checklist\n\n- tests green\n- docs gate", back: "/s/demo-set", item: { kind: "notes", id: "n-4" } };
      return { title: "Launch notes", members: [{ title: "Release checklist", href: "/s/demo-set/1" }, { title: "Invite flow", href: "/s/demo-set/2" }], item: { kind: "collections", id: "c-1" } };
    }
    if (!/^fx\d+k9q2m7x4tz$/.test(secret) && secret !== "demo") throw new ServerError("not_found", "that link does not work");
    return { title: "Release checklist", markdown: "# Release checklist\n\n- tests green\n- docs gate\n- changelog\n\nShipping is Thursday; the rest can wait.", item: { kind: "notes", id: "n-4" } };
  }
}

/** The one client the screens use: the real server's when one is chosen, else the fixtures. */
export const sharing: SharingClient = serverMode() ? new RealSharing() : new FixtureSharing();
