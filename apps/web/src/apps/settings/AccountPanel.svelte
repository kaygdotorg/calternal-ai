<script lang="ts">
  // The Account page on the real server: who you are (read only: no operation changes a name yet), your passkeys (add, rename, remove; the last one cannot be removed) and the places you are signed in
  // (revoke one, or sign out everywhere). Changing how someone signs in asks for a fresh passkey first (the server says so; account.* repeats the change after it).
  import { Button } from "@calternal/ui";
  import { account, signOut, type InstallationRow, type Me, type Passkey } from "../../server/auth";
  import { say } from "../../server/say";
  import "./panel.css";
  let me = $state<Me | null>(null), keys = $state<Passkey[]>([]), places = $state<InstallationRow[]>([]);
  let error = $state(""), busy = $state(false), renaming = $state<string | null>(null), draft = $state("");
  const when = (ms: number | null) => (ms ? new Date(ms).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" }) : "never");
  async function load() {
    try { [me, keys, places] = await Promise.all([account.me(), account.passkeys(), account.installations()]); error = ""; }
    catch (e) { error = say(e, "load your account"); }
  }
  async function act(f: () => Promise<unknown>) {
    busy = true; error = "";
    try { await f(); await load(); } catch (e) { error = say(e); }
    busy = false;
  }
  const everywhere = () => act(async () => { for (const p of places.filter((x) => !x.current)) await account.revokeInstallation(p.id); await signOut(); location.replace(import.meta.env.BASE_URL); });
  void load();
</script>

<div class="sp">
  <h2>You</h2>
  <div class="r"><span class="n"><span class="t">{me?.user.name ?? "…"}</span><span class="h">{me ? `${me.user.username} · ${me.user.role}` : ""}</span></span></div>
  <h2>Passkeys</h2>
  {#each keys as k (k.id)}
    <div class="r" data-setting="account.passkey.{k.id}">
      {#if renaming === k.id}
        <input class="field" aria-label="Passkey name" bind:value={draft} onkeydown={(e) => e.key === "Enter" && act(async () => { await account.rename(k.id, draft); renaming = null; })} />
        <span class="a"><Button text="Save" variant="primary" disabled={busy || !draft.trim()} onclick={() => act(async () => { await account.rename(k.id, draft); renaming = null; })} /><Button text="Cancel" variant="quiet" onclick={() => (renaming = null)} /></span>
      {:else}
        <span class="n"><span class="t">{k.name}</span><span class="h">Added {when(k.created_at)} · last used {when(k.last_used_at)}</span></span>
        <span class="a"><Button text="Rename" variant="quiet" onclick={() => { renaming = k.id; draft = k.name; }} /><Button text="Remove" variant="quiet" disabled={busy || keys.length < 2} onclick={() => confirm(`Remove the passkey "${k.name}"?`) && act(() => account.remove(k.id))} /></span>
      {/if}
    </div>
  {/each}
  <div class="r"><span class="n"><span class="t">Add a passkey</span><span class="h">On this device, or one nearby.</span></span><Button text="Add a passkey…" disabled={busy} onclick={() => act(() => account.addPasskey(`Passkey ${keys.length + 1}`))} /></div>
  <h2>Signed in on</h2>
  {#each places as p (p.id)}
    <div class="r" data-setting="account.installation.{p.id}">
      <span class="n"><span class="t">{p.name}{p.current ? " (this browser)" : ""}</span><span class="h">Since {when(p.created_at)} · last used {when(p.last_seen_at)}</span></span>
      {#if !p.current}<Button text="Sign out" variant="quiet" disabled={busy} onclick={() => act(() => account.revokeInstallation(p.id))} />{/if}
    </div>
  {/each}
  <div class="r"><span class="n"><span class="t">Sign out</span><span class="h">Ends the session in this browser.</span></span><Button text="Sign out" variant="secondary" disabled={busy} onclick={() => act(async () => { await signOut(); location.replace(import.meta.env.BASE_URL); })} /></div>
  <div class="r"><span class="n"><span class="t">Sign out everywhere</span><span class="h">Ends every session, including this one.</span></span><Button text="Sign out everywhere…" variant="secondary" disabled={busy} onclick={() => confirm("Sign out of every browser and app, including this one?") && everywhere()} /></div>
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
