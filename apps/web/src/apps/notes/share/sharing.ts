/**
 * What the Share sheet, the "Shared with me" list and the guest page need from a backend: the `sharing.*` operations (issue #60), with exactly their shapes. Two implementations: fixtures (the default, in memory) and,
 * when #60 lands, the real server behind the same interface. The screens only see `SharingClient`.
 *
 * Open on the backend's side (assumed here, to be confirmed on #60): the grant row's fields, and the values of `space_access` and of the caller's own `permission`.
 */
import { ServerError } from "../../../server/client";

/** What a share points at: a note, or a collection (a View). */
export interface ItemRef { kind: "notes" | "collections"; id: string }
/** Who a grant is for: one account or one group. A link has no audience. */
export type Audience = { account_id: string } | { group_id: string };
/** What an audience may do. Links are view-only in this slice. */
export type Permission = "view" | "edit";
/** One grant: a person, a group or (no `audience`) a link. `name` is the person's or group's display name, or the link's label; `expires_at` is Unix milliseconds, null = Never. */
export interface Grant { grant_id: string; audience?: Audience; name: string; permission: Permission; expires_at: number | null }
/** One page of `sharing.list`: who the item is shared with, what the caller may manage, and (assumed) what everyone in the space has. */
export interface ShareList {
  /** The caller's own standing on the item. */
  permission: Permission | "owner";
  can_manage_audiences: boolean;
  can_manage_links: boolean;
  /** What every member of the item's space can do with it, or null when the space has no access. */
  space_access: Permission | null;
  grants: Grant[];
  next_cursor?: string;
}
/** A person or group the caller can add. */
export interface Found { audience: Audience; name: string }
/** One row of "Shared with me": the item, its title, the sharer's display name (never an email), the caller's permission and whether it changed since the caller last opened it. */
export interface Incoming { item: ItemRef; title: string; shared_by: string; permission: Permission; unread: boolean }

/** What a link's secret opens: the item alone, as text, and who shared it (a display name). Assumed shape of the guest read (not in #60's list yet): it needs no sign-in, and a wrong, revoked or expired secret is `not_found`. */
export interface GuestItem { item: ItemRef; title: string; markdown: string; shared_by: string }

/** The `sharing.*` operations. Failures are `ServerError` (`forbidden` when the caller may not manage that part, `not_found` for a grant that is gone or a link that expired). */
export interface SharingClient {
  list(item: ItemRef, cursor?: string): Promise<ShareList>;
  /** Accounts and groups whose name starts with `query`, not yet granted on the item. */
  audiences(item: ItemRef, query: string, cursor?: string): Promise<{ audiences: Found[]; next_cursor?: string }>;
  grant(item: ItemRef, audience: Audience, permission: Permission): Promise<Grant>;
  link_create(item: ItemRef, label: string, expires_at?: number | null): Promise<Grant>;
  link_update(item: ItemRef, grant_id: string, label?: string, expires_at?: number | null): Promise<Grant>;
  /** The link's address, which carries its secret: asked for when copied or drawn as a code, never listed. */
  link_copy(item: ItemRef, grant_id: string): Promise<{ url: string }>;
  revoke(item: ItemRef, grant_id: string): Promise<void>;
  shared_with_me(kind: ItemRef["kind"], cursor?: string): Promise<{ items: Incoming[]; next_cursor?: string }>;
  set_link_policy(space_id: string, editors_can_manage_links: boolean): Promise<void>;
  /** Opens a link's secret without signing in: just that item, read-only. */
  guest_read(secret: string): Promise<GuestItem>;
}

/** The same key for an audience, in a list or a map. */
export const audienceKey = (a: Audience): string => ("account_id" in a ? `a:${a.account_id}` : `g:${a.group_id}`);
/** Whether an audience is a group. */
export const isGroup = (a: Audience | undefined): boolean => !!a && "group_id" in a;

/** One plain sentence on who can see the item, from what the list says: named people and groups by what they can do, then links, then the space. */
export function sentenceOf(list: Pick<ShareList, "grants" | "space_access">): string {
  const join = (a: string[]) => (a.length <= 1 ? (a[0] ?? "") : `${a.slice(0, -1).join(", ")} and ${a.at(-1)}`);
  const named = list.grants.filter((g) => g.audience);
  const names = (p: Permission) => named.filter((g) => g.permission === p).map((g) => g.name);
  const parts: string[] = [];
  if (list.space_access) parts.push(`Everyone in this space can ${list.space_access}`);
  if (names("edit").length) parts.push(`${join(names("edit"))} can edit`);
  if (names("view").length) parts.push(`${join(names("view"))} can view`);
  if (list.grants.some((g) => !g.audience)) parts.push("Anyone with a link can view");
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
  ];
  async list(item: ItemRef): Promise<ShareList> {
    const as = this.as;
    return { permission: as === "viewer" ? "view" : as === "editor" ? "edit" : "owner", can_manage_audiences: as !== "viewer", can_manage_links: as === "owner" || as === "manager", space_access: null, grants: this.of(item) };
  }
  async audiences(item: ItemRef, query: string) {
    const have = new Set(this.of(item).map((g) => g.audience && audienceKey(g.audience)));
    const q = query.trim().toLowerCase();
    return { audiences: PEOPLE.filter((p) => !have.has(audienceKey(p.audience)) && p.name.toLowerCase().includes(q)) };
  }
  async grant(item: ItemRef, audience: Audience, permission: Permission): Promise<Grant> {
    const list = this.of(item).slice();
    const at = list.findIndex((g) => g.audience && audienceKey(g.audience) === audienceKey(audience));
    const name = PEOPLE.find((p) => audienceKey(p.audience) === audienceKey(audience))?.name ?? "Someone";
    const g: Grant = { grant_id: at >= 0 ? list[at]!.grant_id : `g-${++this.n}`, audience, name, permission, expires_at: null };
    if (at >= 0) list[at] = g; else list.push(g);
    this.grants.set(this.key(item), list);
    return g;
  }
  async link_create(item: ItemRef, label: string, expires_at: number | null = null): Promise<Grant> {
    const g: Grant = { grant_id: `l-${++this.n}`, name: label, permission: "view", expires_at };
    this.secrets.set(g.grant_id, `fx${this.n}k9q2m7x4tz`);
    this.grants.set(this.key(item), [...this.of(item), g]);
    return g;
  }
  async link_update(item: ItemRef, grant_id: string, label?: string, expires_at?: number | null): Promise<Grant> {
    const list = this.of(item).map((g) => (g.grant_id === grant_id ? { ...g, ...(label !== undefined ? { name: label } : {}), ...(expires_at !== undefined ? { expires_at } : {}) } : g));
    const g = list.find((x) => x.grant_id === grant_id);
    if (!g) throw new ServerError("not_found", "that link is gone");
    this.grants.set(this.key(item), list);
    return g;
  }
  async link_copy(_item: ItemRef, grant_id: string): Promise<{ url: string }> {
    const s = this.secrets.get(grant_id);
    if (!s) throw new ServerError("not_found", "that link is gone");
    return { url: `${location.origin}/s/${s}` };
  }
  async revoke(item: ItemRef, grant_id: string): Promise<void> { this.grants.set(this.key(item), this.of(item).filter((g) => g.grant_id !== grant_id)); }
  async shared_with_me(_kind: ItemRef["kind"]) { return { items: this.incoming }; }
  async set_link_policy(): Promise<void> {}
  async guest_read(secret: string): Promise<GuestItem> {
    // Any secret the fixtures made opens the first note; `?secret=expired` style secrets ("gone") do not.
    if (!/^fx\d+k9q2m7x4tz$/.test(secret) && secret !== "demo") throw new ServerError("not_found", "that link does not work");
    return { item: { kind: "notes", id: "n-4" }, title: "Release checklist", markdown: "# Release checklist\n\n- tests green\n- docs gate\n- changelog\n\nShipping is Thursday; the rest can wait.", shared_by: "Maya Okafor" };
  }
}

/** The one client the screens use. */
export const sharing: SharingClient = new FixtureSharing();
