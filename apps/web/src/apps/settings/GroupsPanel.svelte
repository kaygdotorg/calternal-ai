<script lang="ts">
  // Admin: the server's groups. Make a local group, rename or delete it (deleting removes every grant it held), and put people in or take them out. Directory groups (another `source`) are shown, not edited.
  import { Button } from "@calternal/ui";
  import * as people from "../../server/people";
  import { say } from "../../server/say";
  import "./panel.css";
  let list = $state<people.Group[]>([]), everyone = $state<people.Person[]>([]), error = $state(""), busy = $state(false), loaded = $state(false), name = $state(""), open = $state<string | null>(null);
  const nameOf = (id: string) => everyone.find((p) => p.account_id === id)?.name ?? id.slice(0, 8);
  async function load() { try { [list, everyone] = await Promise.all([people.groups(), people.people().catch(() => [])]); error = ""; loaded = true; } catch (e) { error = say(e, "load the groups"); } }
  async function act(f: () => Promise<unknown>) { busy = true; error = ""; try { await f(); } catch (e) { error = say(e); } await load(); busy = false; }
  const rename = (x: people.Group) => { const n = prompt("New name for the group", x.name)?.trim(); if (n && n !== x.name) void act(() => people.renameGroup(x.group_id, n)); };
  void load();
</script>

<div class="sp">
  <div class="r"><span class="n"><label class="t" for="g-new">Make a group</label><input id="g-new" class="field" bind:value={name} placeholder="Name, for example Design" autocomplete="off" /></span>
    <Button text="Make it" variant="primary" disabled={busy || !name.trim()} onclick={() => act(async () => { await people.createGroup(name.trim()); name = ""; })} /></div>
  {#each list as x (x.group_id)}
    <div class="r" data-setting="server.group.{x.group_id}"><span class="n"><span class="t">{x.name}</span><span class="h">{x.source === "local" ? "Local" : x.source} · {x.members.length} {x.members.length === 1 ? "person" : "people"}{x.members_next_cursor ? "+" : ""}</span></span>
      <span class="a"><Button text={open === x.group_id ? "Done" : "People"} variant="quiet" onclick={() => (open = open === x.group_id ? null : x.group_id)} />{#if x.source === "local"}<Button text="Rename" variant="quiet" onclick={() => rename(x)} /><Button text="Delete" variant="quiet" disabled={busy} onclick={() => confirm(`Delete the group "${x.name}"? Every grant it held is removed.`) && act(() => people.deleteGroup(x.group_id))} />{/if}</span></div>
    {#if open === x.group_id}
      {#each x.members as id (id)}<div class="r"><span class="t">{nameOf(id)}</span>{#if x.source === "local"}<Button text="Remove" variant="quiet" disabled={busy} onclick={() => act(() => people.removeFromGroup(x.group_id, id))} />{/if}</div>{/each}
      {#if x.source === "local"}{#each everyone.filter((p) => !p.disabled && !x.members.includes(p.account_id)) as p (p.account_id)}<div class="r"><span class="h">{p.name}</span><Button text="Add" variant="quiet" disabled={busy} onclick={() => act(() => people.addToGroup(x.group_id, p.account_id))} /></div>{/each}{/if}
    {/if}
  {/each}
  {#if loaded && !list.length && !error}<div class="r"><span class="h">No groups yet. A group gives one role in a space to everyone in it, now and later.</span></div>{/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
