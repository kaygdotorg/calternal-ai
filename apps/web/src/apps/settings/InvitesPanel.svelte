<script lang="ts">
  // Invitations to the current space: make one (a role and how long it lasts), copy the link of a live one, revoke an unused one. A link is a credential: it is shown only here, to people who may manage it, and the
  // person who opens it makes their own account and passkey and lands in their own space with this one added.
  import { Button, Selector, VirtualList } from "@calternal/ui";
  import * as people from "../../server/people";
  import { say } from "../../server/say";
  import { spaces } from "../../shell/spaces.svelte";
  import { capitalise, copyText, shortDate } from "../../lib/format";
  import "./panel.css";
  let list = $state<people.Invite[]>([]), error = $state(""), busy = $state(false), loaded = $state(false), role = $state<people.GrantRole>("editor"), days = $state("7"), copied = $state("");
  let past = $state(false), pg = $state<people.Pager<people.Invite> | null>(null), loading = false;
  const space = $derived(spaces.current.id);
  const isLive = (i: people.Invite) => !i.revoked_at && !i.used_at && i.expires_at >= Date.now();
  const open = $derived(list.filter(isLive)), earlier = $derived(list.filter((i) => !isLive(i)));
  const shown = $derived(past ? [...open, ...earlier] : open);
  const isOwner = $derived(spaces.current.role === "owner");
  const ROLES = $derived([{ id: "reader", label: "Reader" }, { id: "editor", label: "Editor" }, ...(isOwner ? [{ id: "manager", label: "Manager" }] : [])]);
  const WHEN = [{ id: "1", label: "1 day" }, { id: "7", label: "7 days" }, { id: "30", label: "30 days" }];
  const standing = (i: people.Invite) => (i.revoked_at ? "Revoked" : i.used_at ? "Used" : i.expires_at < Date.now() ? "Expired" : `Open until ${shortDate(i.expires_at)}`);
  async function load() { try { const fresh = people.invitePager(space); list = await fresh.next(); pg = fresh; error = ""; loaded = true; } catch (e) { error = say(e, "load the invitations"); } }
  async function more() {
    if (!pg || pg.done || loading) return;
    loading = true;
    try {
      // A page of used invitations adds nothing to the open ones shown, so keep reading until the list grows or ends (the window only asks again when the count changes).
      const before = shown.length;
      for (let n = 0; n < 50 && !pg.done; n++) { list = [...list, ...(await pg.next())]; if (shown.length > before) break; }
    } catch (e) { error = say(e, "load more invitations"); }
    loading = false;
  }
  async function act(f: () => Promise<unknown>) { busy = true; error = ""; try { await f(); } catch (e) { error = say(e); } await load(); busy = false; }
  async function copy(i: people.Invite) { if (await copyText(i.link!)) { copied = i.invite_id; setTimeout(() => (copied = ""), 2000); } else error = "Could not copy. Select the link and copy it."; }
  $effect(() => { void space; void load(); });
</script>

<div class="sp">
  <div class="r"><span class="n"><span class="t">Invite someone</span><span class="h">They open the link, make a passkey, and join with this role.</span>
    <Selector class="row-selector" label="Role" options={ROLES.map((r) => ({ value: r.id, label: r.label }))} value={role} onchange={(v) => (role = v as people.GrantRole)} />
    <Selector class="row-selector" label="Lasts" options={WHEN.map((r) => ({ value: r.id, label: r.label }))} value={days} onchange={(v) => (days = v)} /></span>
    <Button class="settings-action" text="Make an invitation" variant="primary" disabled={busy} onclick={() => act(() => people.createInvite(space, role, Date.now() + Number(days) * 86400000))} /></div>
  <VirtualList inside count={shown.length} estimate={90} overscan={600} label="Invitations" onrange={(_, end) => { if (end >= shown.length - 20) void more(); }}>
    {#snippet row(ix)}{@const i = shown[ix]}{#if i}
    <div class="r" data-setting="space.invite.{i.invite_id}">
      <span class="n"><span class="t">{capitalise(i.role)}</span><span class="h">{standing(i)}</span>{#if i.link}<code class="cmd">{i.link}</code>{/if}</span>
      <span class="a">{#if i.link}<Button text={copied === i.invite_id ? "Copied" : "Copy link"} variant="secondary" onclick={() => copy(i)} /><Button text="Revoke" variant="quiet" disabled={busy} onclick={() => confirm("Revoke this invitation? The link stops working.") && act(() => people.revokeInvite(space, i.invite_id))} />{/if}</span>
    </div>
    {/if}{/snippet}
  </VirtualList>
  {#if pg && !pg.done}<div class="r"><span class="h">Loading more…</span></div>{/if}
  {#if earlier.length}<div class="r"><span class="h">{earlier.length} earlier {earlier.length === 1 ? "invitation" : "invitations"} (used, revoked or expired)</span><Button text={past ? "Hide them" : "Show them"} variant="quiet" onclick={() => (past = !past)} /></div>{/if}
  {#if loaded && !list.length && !error}<div class="r"><span class="h">No invitations yet. An invitation is a one-use link for someone who has no account here.</span></div>{/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
