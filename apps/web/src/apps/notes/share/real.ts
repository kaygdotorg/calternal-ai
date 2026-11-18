/**
 * The real server's `SharingClient` (issue #60): each `sharing.*` operation is one `POST /api/v1/operations` call through the shared `ServerClient`, and the guest read is the link address itself fetched with
 * `Accept: application/json` (no sign-in; the session cookie, if there is one, is what lets the server add `item`). Fixtures (`FixtureSharing`) stay the default until a server is chosen (`serverMode()`).
 * Cost: one request per call; nothing is cached here, the sheet reads the list again after every change.
 */
import { ServerError, server } from "../../../server/client";
import { href } from "../../../lib/route.svelte";
import type { Audience, Found, Guest, Incoming, ItemRef, LinkGrant, NamedGrant, Permission, ShareList, SharingClient } from "./sharing";

/** Drops the keys that are undefined, so an optional input is absent rather than null. */
const input = (o: Record<string, unknown>): Record<string, unknown> => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined));

/** The sharing operations against the real server. */
export class RealSharing implements SharingClient {
  list(item: ItemRef, cursor?: string) { return server.operate<ShareList>("sharing.list", input({ item, cursor })); }
  audiences(item: ItemRef, query: string, cursor?: string) { return server.operate<{ audiences: Found[]; next_cursor?: string }>("sharing.audiences", input({ item, query, cursor })); }
  grant(item: ItemRef, audience: Audience, permission: Permission) { return server.operate<NamedGrant>("sharing.grant", { item, audience, permission }); }
  link_create(item: ItemRef, label: string, expires_at?: number | null) { return server.operate<LinkGrant>("sharing.link_create", input({ item, label, expires_at })); }
  link_update(item: ItemRef, grant_id: string, label?: string, expires_at?: number | null) { return server.operate<LinkGrant>("sharing.link_update", input({ item, grant_id, label, expires_at })); }
  link_copy(item: ItemRef, grant_id: string) { return server.operate<{ url: string }>("sharing.link_copy", { item, grant_id }); }
  async revoke(item: ItemRef, grant_id: string): Promise<void> { await server.operate("sharing.revoke", { item, grant_id }); }
  shared_with_me(kind: ItemRef["kind"], cursor?: string) { return server.operate<{ items: Incoming[]; next_cursor?: string }>("sharing.shared_with_me", input({ kind, cursor })); }
  async set_link_policy(space_id: string, editors_can_manage_links: boolean): Promise<void> { await server.operate("sharing.set_link_policy", { space_id, editors_can_manage_links }); }
  rescue(title: string, changed_blocks: string[], request_id: string) { return server.operate<{ note: { id: string; title: string } }>("sharing.rescue", { title, changed_blocks }, request_id); }
  mark_seen(item: ItemRef) { return server.operate<{ seen: true }>("sharing.mark_seen", { item }); }
  async guest(path: string): Promise<Guest> {
    let res: Response;
    try { res = await fetch(href(path), { headers: { Accept: "application/json" }, credentials: "same-origin" }); } catch { throw new ServerError("unavailable", "can't reach the server"); }
    // Every target that does not work (wrong, revoked, expired, trashed) is a 404 and says nothing more.
    if (res.status === 404) throw new ServerError("not_found", "that link does not work", 404);
    if (!res.ok) throw new ServerError(res.status >= 500 ? "internal" : "invalid_input", "the link could not be read", res.status);
    return (await res.json()) as Guest;
  }
}
