<script lang="ts">
  // Members of the current space on the real server: each member with a role you can change (if you may) and a Remove. The server lists accounts by id only (no names yet), so a row shows
  // "You" or the start of the id; a name column waits for an operation that returns people's names (docs/ui-shell-status.md, server mode).
  import { Capsule } from "@calternal/ui";
  import { Button } from "@calternal/ui";
  import { server } from "../../server/client";
  import { say } from "../../server/say";
  import { spaces } from "../../shell/spaces.svelte";
  import "./panel.css";
  type Role = "reader" | "editor" | "manager" | "owner";
  let members = $state<{ account_id: string; role: Role }[]>([]), me = $state(""), error = $state(""), busy = $state(false);
  const space = $derived(spaces.current.id);
  const ROLES = [{ id: "reader", label: "Reader" }, { id: "editor", label: "Editor" }, { id: "manager", label: "Manager" }];
  async function load() {
    try {
      const [m, who] = await Promise.all([server.operate<{ members: { account_id: string; role: Role }[] }>("spaces.members", { space_id: space }), import("../../server/auth").then((a) => a.account.me())]);
      members = m.members; me = who.user.id; error = "";
    } catch (e) { error = say(e); }
  }
  async function act(op: string, input: Record<string, unknown>) {
    busy = true; error = "";
    try { await server.operate(op, { space_id: space, ...input }); await load(); } catch (e) { error = say(e); await load(); }
    busy = false;
  }
  const mine = $derived(members.find((m) => m.account_id === me)?.role);
  const canManage = $derived(mine === "owner" || mine === "manager");
  $effect(() => { void space; void load(); });
</script>

<div class="sp">
  {#each members as m (m.account_id)}
    <div class="r" data-setting="space.member.{m.account_id}">
      <span class="n"><span class="t">{m.account_id === me ? "You" : m.account_id.slice(0, 8)}</span><span class="h">{m.role === "owner" ? "Owner" : m.role[0]!.toUpperCase() + m.role.slice(1)}</span></span>
      {#if canManage && m.role !== "owner"}
        <Capsule label="Role of {m.account_id.slice(0, 8)}" kind="radio" glass={false} items={ROLES} value={m.role} onchange={(v) => act("spaces.set_role", { account_id: m.account_id, role: v })} />
        <Button text="Remove" variant="quiet" disabled={busy} onclick={() => confirm("Remove this member from the space?") && act("spaces.remove_member", { account_id: m.account_id })} />
      {/if}
    </div>
  {/each}
  {#if members.length === 1 && !error}<p class="e" style="color: var(--ink-3)">Only you are in this space. People you add appear here with their role.</p>{/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
