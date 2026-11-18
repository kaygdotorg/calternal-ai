<script lang="ts">
  // The current space's people and groups: each person with a role you can change (if you may), Remove, and "Make owner" (the owner only); groups with their role; and the two ways to add: a person or a group already on this
  // server, or an invitation for someone who is not (Invites page). A grant can widen a role, never narrow it, so lowering one removes the person first (the server says so in words).
  import { Button, Selector, VirtualList } from "@calternal/ui";
  import { account } from "../../server/auth";
  import * as people from "../../server/people";
  import { say } from "../../server/say";
  import { spaces } from "../../shell/spaces.svelte";
  import { capitalise } from "../../lib/format";
  import "./panel.css";
  let g = $state<people.SpaceGrants>({ members: [], groups: [] }), me = $state(""), error = $state(""), busy = $state(false), loaded = $state(false);
  let adding = $state<"person" | "group" | null>(null), pool = $state<{ id: string; name: string }[]>([]), role = $state<people.GrantRole>("editor"), find = $state("");
  const space = $derived(spaces.current.id);
  const mine = $derived(g.members.find((m) => m.account_id === me)?.role);
  const canManage = $derived(mine === "owner" || mine === "manager");
  const ROLES = [{ id: "reader", label: "Reader" }, { id: "editor", label: "Editor" }, { id: "manager", label: "Manager" }];
  // The first page, then the next as the person scrolls toward the end (a server page is 50).
  let pg = $state<people.Pager<people.Member> | null>(null), loading = false;
  async function load() {
    try {
      const fresh = people.memberPager(space, (groups) => (g.groups = groups));
      const [first, who] = await Promise.all([fresh.next(), account.me().then((m) => m.user.id)]);
      g.members = first; me = who; pg = fresh; error = ""; loaded = true;
    } catch (e) { error = say(e, "load the members"); }
  }
  async function more() {
    if (!pg || pg.done || loading) return;
    loading = true;
    try { g.members = [...g.members, ...(await pg.next())]; } catch (e) { error = say(e, "load more people"); }
    loading = false;
  }
  async function act(f: () => Promise<unknown>) { busy = true; error = ""; try { await f(); } catch (e) { error = say(e); } await load(); busy = false; }
  async function open(kind: "person" | "group") {
    adding = kind; find = ""; pool = [];
    try {
      if (kind === "person") { const have = new Set(g.members.map((m) => m.account_id)); pool = (await people.people()).filter((p) => !p.disabled && !have.has(p.account_id)).map((p) => ({ id: p.account_id, name: p.name })); }
      else { const have = new Set(g.groups.map((x) => x.group_id)); pool = (await people.groups()).filter((x) => !have.has(x.group_id)).map((x) => ({ id: x.group_id, name: x.name })); }
    } catch (e) { error = say(e, "list them"); adding = null; }
  }
  const shown = $derived(pool.filter((p) => p.name.toLowerCase().includes(find.trim().toLowerCase())));
  const add = (id: string) => act(async () => { await people.addMember(space, adding === "person" ? { account_id: id } : { group_id: id }, role); adding = null; });
  $effect(() => { void space; void load(); });
</script>

<div class="sp">
  <VirtualList inside count={g.members.length} estimate={64} overscan={600} label="People" onrange={(_, end) => { if (end >= g.members.length - 20) void more(); }}>
    {#snippet row(ix)}{@const m = g.members[ix]}{#if m}
    <div class="r" class:col={canManage && m.role !== "owner" && m.account_id !== me} data-setting="space.member.{m.account_id}">
      <span class="n"><span class="t">{m.account_id === me ? `${m.name} (you)` : m.name}</span><span class="h">{capitalise(m.role)}</span></span>
      {#if canManage && m.role !== "owner" && m.account_id !== me}
        <span class="a">
          <Selector label="Role of {m.name}" fill={false} options={(mine === "owner" ? ROLES : ROLES.slice(0, 2)).map((r) => ({ value: r.id, label: r.label }))} value={m.role} onchange={(v) => act(() => people.setRole(space, m.account_id, v as people.GrantRole))} />
          {#if mine === "owner"}<Button text="Make owner" variant="quiet" disabled={busy} onclick={() => confirm(`Make ${m.name} the owner of this space? You become a manager.`) && act(() => people.transfer(space, m.account_id))} />{/if}
          <Button text="Remove" variant="quiet" disabled={busy} onclick={() => confirm(`Remove ${m.name} from this space?`) && act(() => people.removeMember(space, m.account_id))} />
        </span>
      {/if}
    </div>
    {/if}{/snippet}
  </VirtualList>
  {#if pg && !pg.done}<div class="r"><span class="h">Loading more people…</span></div>{/if}
  {#if g.groups.length}<h2>Groups</h2>{/if}
  {#each g.groups as x (x.group_id)}
    <div class="r" data-setting="space.group.{x.group_id}"><span class="n"><span class="t">{x.name}</span><span class="h">{capitalise(x.role)} · {x.member_count} {x.member_count === 1 ? "person" : "people"}</span></span></div>
  {/each}
  {#if loaded && g.members.length === 1 && !error}<p class="e" style="color: var(--ink-3)">Only you are in this space. Add a person who has an account here, a group, or send an invitation from the Invites page.</p>{/if}
  {#if canManage}
    <div class="r"><span class="n"><span class="t">Add to this space</span><span class="h">A person or a group already on this server, with a role.</span></span>
      <span class="a"><Button text="Add a person…" disabled={busy} onclick={() => open("person")} /><Button text="Add a group…" variant="secondary" disabled={busy} onclick={() => open("group")} /></span></div>
  {/if}
  {#if adding}
    <div class="r"><span class="n">
      <label class="t" for="m-find">{adding === "person" ? "Choose a person" : "Choose a group"}</label>
      <input id="m-find" class="field" placeholder="Search" bind:value={find} autocomplete="off" />
      <Selector label="Role to give" fill={false} options={(mine === "owner" ? ROLES : ROLES.slice(0, 2)).map((r) => ({ value: r.id, label: r.label }))} value={role} onchange={(v) => (role = v as people.GrantRole)} />
    </span></div>
    {#each shown.slice(0, 8) as p (p.id)}<div class="r"><span class="t">{p.name}</span><Button text="Add as {role}" variant="quiet" disabled={busy} onclick={() => add(p.id)} /></div>{:else}<div class="r"><span class="h">{pool.length ? "Nobody matches." : adding === "person" ? "Everyone on this server is already here." : "No other groups."}</span></div>{/each}
    <div class="r"><Button text="Cancel" variant="quiet" onclick={() => (adding = null)} /></div>
  {/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
