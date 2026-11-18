<script lang="ts">
  // The Account page on the real server: who you are (read only: no operation changes a name yet), your passkeys (add, rename, remove; the last one cannot be removed) and the places you are signed in
  // (revoke one, or sign out everywhere). Changing how someone signs in asks for a fresh passkey first (the server says so; account.* repeats the change after it).
  import { Button } from "@calternal/ui";
  import { account, changeUsername, regenerateRecoveryCodes, signOut, type InstallationRow, type Me, type Passkey } from "../../server/auth";
  import { usernameProblem } from "../../shell/onboarding/account-rules";
  import EmailForm from "../../shell/onboarding/EmailForm.svelte";
  import RecoveryCodes from "../../shell/onboarding/RecoveryCodes.svelte";
  import { say } from "../../server/say";
  import { setName } from "../../server/people";
  import { shortDate } from "../../lib/format";
  import "./panel.css";
  let me = $state<Me | null>(null), keys = $state<Passkey[]>([]), places = $state<InstallationRow[]>([]);
  let error = $state(""), busy = $state(false), renaming = $state<string | null>(null), draft = $state("");
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
  let name = $state("");
  $effect(() => { if (me && !name) name = me.user.name; });
  const saveName = () => act(() => setName(name.trim()));
  let uname = $state(""), codes = $state<string[] | null>(null), editingEmail = $state(false);
  $effect(() => { if (me && !uname) uname = me.user.username ?? ""; });
  const uproblem = $derived(uname.trim() && uname.trim().toLowerCase() !== (me?.user.username ?? "") ? usernameProblem(uname) : null);
  const saveUsername = () => act(async () => { await changeUsername(uname); });
  const newCodes = () => act(async () => { codes = await regenerateRecoveryCodes(); });
  const deviceLabel = () => "Second passkey";
  const EMAIL: Record<string, string> = { missing: "No email address.", unverified: "Not confirmed.", verified: "Confirmed." };
  void load();
</script>

<div class="sp">
  <h2>You</h2>
  <div class="r"><span class="n"><label class="t" for="acc-name">Name</label><input id="acc-name" class="field" bind:value={name} onkeydown={(e) => e.key === "Enter" && saveName()} autocomplete="name" /><span class="h">{me ? `${me.user.username} · ${me.user.role === "admin" ? "operator" : "member"}` : ""}</span></span>
    <Button text="Save" variant="secondary" disabled={busy || !name.trim() || name.trim() === me?.user.name} onclick={saveName} /></div>
  <div class="r"><span class="n"><label class="t" for="acc-user">Username</label><input id="acc-user" class="field" bind:value={uname} autocomplete="username" autocapitalize="none" spellcheck="false" onkeydown={(e) => e.key === "Enter" && !uproblem && saveUsername()} /><span class="h" class:e={!!uproblem}>{uproblem ?? "3 to 32 letters, digits or hyphens."}</span></span>
    <Button class="settings-action" text="Change" variant="secondary" disabled={busy || !!uproblem || !uname.trim() || uname.trim().toLowerCase() === me?.user.username} onclick={saveUsername} /></div>
  <div class="r"><span class="n"><span class="t">Email</span><span class="h">{me?.email ? `${me.email}. ${EMAIL[me.email_status ?? "unverified"]}` : "None. Without an email, recovery goes through this server's administrator."}</span>{#if me?.mail_available === false}<span class="h">This server cannot send email, so an address cannot be verified here.</span>{/if}</span>
    {#if !editingEmail}<Button class="settings-action" text={me?.email ? "Change…" : "Add…"} variant="secondary" disabled={busy} onclick={() => (editingEmail = true)} />{/if}</div>
  {#if editingEmail}<div class="r col"><EmailForm mail={me?.mail_available !== false} initial={me?.email ?? ""} oncancel={() => (editingEmail = false)} ondone={() => { editingEmail = false; void load(); }} /></div>{/if}
  <h2>Recovery</h2>
  <div class="r"><span class="n"><span class="t">Recovery codes</span><span class="h">{me?.recovery_codes_configured === false ? "You have none yet." : "Ten single-use codes get you in without a passkey. Making a new set stops the old one working."}</span></span>
    <Button class="settings-action" text="Make new codes…" variant="secondary" disabled={busy} onclick={newCodes} /></div>
  {#if codes}<div class="r col"><RecoveryCodes {codes} label="I have saved them" onsaved={() => (codes = null)} /></div>{/if}
  {#if me?.second_passkey_prompt && keys.length < 2}<div class="r"><span class="n"><span class="t">Add a second passkey</span><span class="h">With one passkey, losing that device means using a recovery code.</span></span><Button class="settings-action" text="Add a passkey…" variant="primary" disabled={busy} onclick={() => act(() => account.addPasskey(deviceLabel()))} /></div>{/if}
  <h2>Passkeys</h2>
  {#each keys as k (k.id)}
    <div class="r" data-setting="account.passkey.{k.id}">
      {#if renaming === k.id}
        <input class="field" aria-label="Passkey name" bind:value={draft} onkeydown={(e) => e.key === "Enter" && act(async () => { await account.rename(k.id, draft); renaming = null; })} />
        <span class="a"><Button text="Save" variant="primary" disabled={busy || !draft.trim()} onclick={() => act(async () => { await account.rename(k.id, draft); renaming = null; })} /><Button text="Cancel" variant="quiet" onclick={() => (renaming = null)} /></span>
      {:else}
        <span class="n"><span class="t">{k.name}</span><span class="h">Added {shortDate(k.created_at)} · last used {shortDate(k.last_used_at)}</span></span>
        <span class="a"><Button text="Rename" variant="quiet" onclick={() => { renaming = k.id; draft = k.name; }} /><Button text="Remove" variant="quiet" disabled={busy || keys.length < 2} onclick={() => confirm(`Remove the passkey "${k.name}"?`) && act(() => account.remove(k.id))} /></span>
      {/if}
    </div>
  {/each}
  <div class="r"><span class="n"><span class="t">Add a passkey</span><span class="h">On this device, or one nearby.</span></span><Button text="Add a passkey…" disabled={busy} onclick={() => act(() => account.addPasskey(`Passkey ${keys.length + 1}`))} /></div>
  <h2>Signed in on</h2>
  {#each places as p (p.id)}
    <div class="r" data-setting="account.installation.{p.id}">
      <span class="n"><span class="t">{p.name}{p.current ? " (this browser)" : ""}</span><span class="h">Since {shortDate(p.created_at)} · last used {shortDate(p.last_seen_at)}</span></span>
      {#if !p.current}<Button text="Sign out" variant="quiet" disabled={busy} onclick={() => act(() => account.revokeInstallation(p.id))} />{/if}
    </div>
  {/each}
  <div class="r"><span class="n"><span class="t">Sign out</span><span class="h">Ends the session in this browser.</span></span><Button text="Sign out" variant="secondary" disabled={busy} onclick={() => act(async () => { await signOut(); location.replace(import.meta.env.BASE_URL); })} /></div>
  <div class="r"><span class="n"><span class="t">Sign out everywhere</span><span class="h">Ends every session, including this one.</span></span><Button text="Sign out everywhere…" variant="secondary" disabled={busy} onclick={() => confirm("Sign out of every browser and app, including this one?") && everywhere()} /></div>
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
