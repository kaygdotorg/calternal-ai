<script lang="ts">
  // Admin: every account on this server with its role, and Disable (ends its sessions; no one is deleted). Only people who may manage people see this page.
  import { Button , VirtualList } from "@calternal/ui";
  import * as people from "../../server/people";
  import { say } from "../../server/say";
  import "./panel.css";
  let pg = $state<people.Pager<people.Person> | null>(null), loading = false;
  let list = $state<people.Person[]>([]), error = $state(""), busy = $state(false), loaded = $state(false);
  async function load() { try { const fresh = people.peoplePager(); list = await fresh.next(); pg = fresh; error = ""; loaded = true; } catch (e) { error = say(e, "load the people"); } }
  async function more() { if (!pg || pg.done || loading) return; loading = true; try { list = [...list, ...(await pg.next())]; } catch (e) { error = say(e, "load more people"); } loading = false; }
  async function disable(p: people.Person) { if (!confirm(`Disable ${p.name}? Their sessions end and they cannot sign in.`)) return; busy = true; try { await people.disable(p.account_id); } catch (e) { error = say(e); } await load(); busy = false; }
  void load();
</script>

<div class="sp">
  <VirtualList inside count={list.length} estimate={64} overscan={600} label="People" onrange={(_, end) => { if (end >= list.length - 20) void more(); }}>
    {#snippet row(ix)}{@const p = list[ix]}{#if p}
    <div class="r" data-setting="server.person.{p.account_id}"><span class="n"><span class="t">{p.name}</span><span class="h">{p.role === "admin" ? "Operator" : "Member"}{p.disabled ? " · Disabled" : ""}</span></span>
      {#if !p.disabled}<Button text="Disable" variant="quiet" disabled={busy} onclick={() => disable(p)} />{/if}</div>
    {/if}{/snippet}
  </VirtualList>
  {#if pg && !pg.done}<div class="r"><span class="h">Loading more…</span></div>{/if}
  {#if loaded && !list.length && !error}<div class="r"><span class="h">No accounts.</span></div>{/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
