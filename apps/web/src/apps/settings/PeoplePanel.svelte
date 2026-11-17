<script lang="ts">
  // Admin: every account on this server with its role, and Disable (ends its sessions; no one is deleted). Only people who may manage people see this page.
  import { Button } from "@calternal/ui";
  import * as people from "../../server/people";
  import { say } from "../../server/say";
  import "./panel.css";
  let more = $state(50);
  let list = $state<people.Person[]>([]), error = $state(""), busy = $state(false), loaded = $state(false);
  async function load() { try { list = await people.people(); error = ""; loaded = true; } catch (e) { error = say(e, "load the people"); } }
  async function disable(p: people.Person) { if (!confirm(`Disable ${p.name}? Their sessions end and they cannot sign in.`)) return; busy = true; try { await people.disable(p.account_id); } catch (e) { error = say(e); } await load(); busy = false; }
  void load();
</script>

<div class="sp">
  {#each list.slice(0, more) as p (p.account_id)}
    <div class="r" data-setting="server.person.{p.account_id}"><span class="n"><span class="t">{p.name}</span><span class="h">{p.role === "admin" ? "Operator" : "Member"}{p.disabled ? " · Disabled" : ""}</span></span>
      {#if !p.disabled}<Button text="Disable" variant="quiet" disabled={busy} onclick={() => disable(p)} />{/if}</div>
  {/each}
  {#if list.length > more}<div class="r"><span class="h">Showing {more} of {list.length} accounts.</span><Button text="Show more" variant="quiet" onclick={() => (more += 50)} /></div>{/if}
  {#if loaded && !list.length && !error}<div class="r"><span class="h">No accounts.</span></div>{/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
