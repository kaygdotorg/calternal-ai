/**
 * People, groups, roles and invitations over the operations the people lane declares (origin/lane/people `contract/operations.json`, docs/people.md). The shapes below are copied from that contract until the
 * generated types for them arrive in `packages/contracts` with the merge, when each interface here is replaced by the generated one. Everything goes through `ServerClient.operate`, so failures are `ServerError`s
 * that say.ts words; nothing here touches the DOM. Lists are cursor-paged (50 rows); `all` follows the cursor for the lists a screen shows whole.
 */
import { server } from "./client";

/** A role a person or group can hold in a space. Owner is held by one person and never granted here. */
export type Role = "reader" | "editor" | "manager" | "owner";
/** A role that can be granted. */
export type GrantRole = Exclude<Role, "owner">;

/** A person in a space: a direct grant. */
export interface Member { account_id: string; name: string; role: Role }
/** A group granted a role in a space. */
export interface GroupGrant { group_id: string; name: string; member_count: number; role: Role }
/** A space's grants: people, groups. */
export interface SpaceGrants { members: Member[]; groups: GroupGrant[] }
/** A role and what it allows. */
export interface RoleInfo { name: string; capabilities: string[] }
/** A server account. */
export interface Person { account_id: string; name: string; role: string; disabled: boolean }
/** A group: local (made here) or from the directory (`source`). */
export interface Group { group_id: string; name: string; source: string; members: string[]; members_next_cursor: string | null }
/** An invitation: one use, one role, an expiry; the link is only present while it is live. */
export interface Invite { invite_id: string; role: Role; created_at: number; expires_at: number; used_at: number | null; revoked_at: number | null; link: string | null }

type Page<K extends string, T> = { [P in K]: T[] } & { next_cursor: string | null };
const op = <T,>(name: string, input: Record<string, unknown> = {}): Promise<T> => server.operate<T>(name, input);

/** Follows a cursor-paged list to its end (bounded: a screen that needs more than 20 pages is a different screen). */
async function all<K extends string, T>(key: K, name: string, input: Record<string, unknown>): Promise<T[]> {
  const out: T[] = [];
  let cursor: string | null = null;
  for (let i = 0; i < 20; i++) {
    const page: Page<K, T> = await op(name, { ...input, ...(cursor ? { cursor } : {}) });
    out.push(...page[key]);
    cursor = page.next_cursor;
    if (!cursor) break;
  }
  return out;
}

/** The space's people and groups with their roles. */
export async function grants(space_id: string): Promise<SpaceGrants> {
  const out: SpaceGrants = { members: [], groups: [] };
  let cursor: string | null = null;
  for (let i = 0; i < 20; i++) {
    const p: SpaceGrants & { next_cursor: string | null } = await op("spaces.members", { space_id, ...(cursor ? { cursor } : {}) });
    out.members.push(...p.members); if (i === 0) out.groups = p.groups; // groups are repeated on each page: the first is enough
    cursor = p.next_cursor;
    if (!cursor) break;
  }
  return out;
}
/** Grants a role to a person or a group (never the owner role). A grant can widen a role, not narrow it. */
export const addMember = (space_id: string, who: { account_id: string } | { group_id: string }, role: GrantRole) => op("spaces.add_member", { space_id, role, ...who });
/** Changes a person's role. */
export const setRole = (space_id: string, account_id: string, role: GrantRole) => op("spaces.set_role", { space_id, account_id, role });
/** Removes a person from the space. */
export const removeMember = (space_id: string, account_id: string) => op("spaces.remove_member", { space_id, account_id });
/** Hands the space to another member (the owner becomes a manager). */
export const transfer = (space_id: string, account_id: string) => op("spaces.transfer", { space_id, account_id });
/** The space roles and what each allows. */
export const roles = async (): Promise<RoleInfo[]> => (await op<{ roles: RoleInfo[] }>("roles.list")).roles;

/** Changes the signed-in person's own name. */
export const setName = (name: string) => op<{ account_id: string; name: string }>("account.set_profile", { name });

/** Every server account (an operator's or a people manager's view). */
export const people = () => all<"people", Person>("people", "people.list", {});
/** Disables an account and ends its sessions. Not undone here. */
export const disable = (account_id: string) => op("people.disable", { account_id });

/** Every group. */
export const groups = () => all<"groups", Group>("groups", "groups.list", {});
/** Makes a local group. */
export const createGroup = (name: string) => op<{ group_id: string }>("groups.create", { name });
/** Renames a local group. */
export const renameGroup = (group_id: string, name: string) => op("groups.rename", { group_id, name });
/** Deletes a local group and every grant it held. */
export const deleteGroup = (group_id: string) => op("groups.delete", { group_id });
/** Adds a person to a local group. */
export const addToGroup = (group_id: string, account_id: string) => op("groups.add_member", { group_id, account_id });
/** Removes a person from a local group. */
export const removeFromGroup = (group_id: string, account_id: string) => op("groups.remove_member", { group_id, account_id });

/** The space's invitations (live ones carry their link). */
export const invites = (space_id: string) => all<"invites", Invite>("invites", "invites.list", { space_id });
/** Makes an invitation for a role that lasts until `expires_at` (Unix milliseconds; the server's default is seven days). */
export const createInvite = (space_id: string, role: GrantRole, expires_at?: number) => op<{ invite_id: string; expires_at: number }>("invites.create", { space_id, role, ...(expires_at ? { expires_at } : {}) });
/** Ends an unused invitation. */
export const revokeInvite = (space_id: string, invite_id: string) => op("invites.revoke", { space_id, invite_id });
/** A signed-in person redeems an invitation's secret (the last part of its link). */
export const redeemInvite = (secret: string) => op<{ space_id: string }>("invites.redeem", { secret });

/** The secret in an invitation link or an address: `…/invites/<secret>`. */
export const inviteSecret = (path: string): string | null => /\/invites\/([^/?#\s]+)/.exec(path)?.[1] ?? null;
