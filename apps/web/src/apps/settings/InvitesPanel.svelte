<script lang="ts">
  // Invitations to the current space: make one (a role and how long it lasts), copy the link of a live one, revoke an unused one. A link is a credential: it is shown only here, to people who may manage it, and the
  // person who opens it makes their own account and passkey and lands in their own space with this one added.
  import { Button, Capsule } from "@calternal/ui";
  import * as people from "../../server/people";
  import { say } from "../../server/say";
  import { spaces } from "../../shell/spaces.svelte";
  import "./panel.css";
  let list = $state<people.Invite[]>([]), error = $state(""), busy = $state(false), loaded = $state(false), role = $state<people.GrantRole>("editor"), days = $state("7"), copied = $state("");
  const space = $derived(spaces.current.id);
  const isOwner = $derived(spaces.current.role === "owner");
  const ROLES = $derived([{ id: "reader", label: "Reader" }, { id: "editor", label: "Editor" }, ...(isOwner ? [{ id: "manager", label: "Manager" }] : [])]);
  const WHEN = [{ id: "1", label: "1 day" }, { id: "7", label: "7 days" }, { id: "30", label: "30 days" }];
  const when = (ms: number) => new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  const standing = (i: people.Invite) => (i.revoked_at ? "Revoked" : i.used_at ? "Used" : i.expires_at < Date.now() ? "Expired" : `Open until ${when(i.expires_at)}`);
  async function load() { try { list = await people.invites(space); error = ""; loaded = true; } catch (e) { error = say(e, "load the invitations"); } }
  async function act(f: () => Promise<unknown>) { busy = true; error = ""; try { await f(); } catch (e) { error = say(e); } await load(); busy = false; }
  async function copy(i: people.Invite) { try { await navigator.clipboard.writeText(i.link!); copied = i.invite_id; setTimeout(() => (copied = ""), 2000); } catch { error = "Could not copy. Select the link and copy it."; } }
  $effect(() => { void space; void load(); });
</script>

<div class="sp">
  <div class="r"><span class="n"><span class="t">Invite someone</span><span class="h">They open the link, make a passkey, and join with this role.</span>
    <Capsule label="Role" kind="radio" glass={false} items={ROLES} value={role} onchange={(v) => (role = v as people.GrantRole)} />
    <Capsule label="Lasts" kind="radio" glass={false} items={WHEN} value={days} onchange={(v) => (days = v)} /></span>
    <Button text="Make an invitation" variant="primary" disabled={busy} onclick={() => act(() => people.createInvite(space, role, Date.now() + Number(days) * 86400000))} /></div>
  {#each list as i (i.invite_id)}
    <div class="r" data-setting="space.invite.{i.invite_id}">
      <span class="n"><span class="t">{i.role[0]!.toUpperCase() + i.role.slice(1)}</span><span class="h">{standing(i)}</span>{#if i.link}<code class="cmd">{i.link}</code>{/if}</span>
      <span class="a">{#if i.link}<Button text={copied === i.invite_id ? "Copied" : "Copy link"} variant="secondary" onclick={() => copy(i)} /><Button text="Revoke" variant="quiet" disabled={busy} onclick={() => confirm("Revoke this invitation? The link stops working.") && act(() => people.revokeInvite(space, i.invite_id))} />{/if}</span>
    </div>
  {/each}
  {#if loaded && !list.length && !error}<div class="r"><span class="h">No invitations yet. An invitation is a one-use link for someone who has no account here.</span></div>{/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
