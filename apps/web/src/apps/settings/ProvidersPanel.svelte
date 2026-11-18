<script lang="ts">
  // Admin > Sign-in providers: the server's opt-in is outside this page (CALTERNAL_ACCOUNT_OIDC_ENABLED=1); when it is off the list is empty and
  // the note says so. Each provider is configured here, then enabled on its own row; only enabled providers show on the sign-in screen.
  import { Button } from "@calternal/ui";
  import { adminProviders, saveProvider, setEnabled, type AdminProvider } from "../../server/oidc";
  import { say } from "../../server/say";
  import "./panel.css";
  let list = $state<AdminProvider[] | null>(null), error = $state(""), busy = $state(false), adding = $state(false);
  let f = $state({ id: "", label: "", issuer: "", client_id: "", client_secret: "" });
  const problem = $derived(!/^[a-z0-9-]{2,32}$/.test(f.id) ? "An id of lowercase letters, digits and hyphens." : !f.label.trim() ? "A name for the button." : !/^https:\/\//.test(f.issuer.trim()) ? "The issuer address, starting with https://." : !f.client_id.trim() || !f.client_secret ? "The client id and secret from the provider." : "");
  async function load() { try { list = await adminProviders(); error = ""; } catch (e) { list = []; error = say(e, "load the providers"); } }
  async function act(go: () => Promise<unknown>) { busy = true; error = ""; try { await go(); await load(); } catch (e) { error = say(e); } busy = false; }
  $effect(() => { void load(); });
  const save = () => act(async () => { await saveProvider({ ...f, label: f.label.trim(), issuer: f.issuer.trim(), client_id: f.client_id.trim() }); f = { id: "", label: "", issuer: "", client_id: "", client_secret: "" }; adding = false; });
</script>

<div class="panel" data-setting="server.providers">
  <p class="h">People can sign in with an outside account (any OpenID Connect provider) as well as a passkey. The server must allow it first: set <code>CALTERNAL_ACCOUNT_OIDC_ENABLED=1</code> and restart. Apple and Google are not offered yet.</p>
  {#if list === null}<p class="h">Loading…</p>
  {:else if !list.length && !adding}<p class="h">No providers are set up.</p>{/if}
  {#each list ?? [] as p (p.id)}
    <div class="r" data-setting="server.provider.{p.id}">
      <span class="n"><span class="t">{p.label}</span><span class="h">{p.issuer} · {p.enabled ? "on the sign-in screen" : "set up, not enabled"}</span></span>
      <Button text={p.enabled ? "Disable" : "Enable"} variant={p.enabled ? "quiet" : "secondary"} disabled={busy} onclick={() => act(() => setEnabled(p.id, !p.enabled))} />
    </div>
  {/each}
  {#if adding}
    <form class="r col" onsubmit={(e) => { e.preventDefault(); if (!problem) void save(); }}>
      <label>Id<input bind:value={f.id} autocomplete="off" spellcheck="false" /></label>
      <label>Button name<input bind:value={f.label} autocomplete="off" /></label>
      <label>Issuer<input bind:value={f.issuer} inputmode="url" autocomplete="off" spellcheck="false" /></label>
      <label>Client id<input bind:value={f.client_id} autocomplete="off" spellcheck="false" /></label>
      <label>Client secret<input type="password" bind:value={f.client_secret} autocomplete="new-password" /></label>
      {#if problem}<p class="h">{problem}</p>{/if}
      <span class="a"><Button text="Cancel" variant="quiet" onclick={() => (adding = false)} /><Button type="submit" text="Save provider" variant="primary" disabled={busy || !!problem} /></span>
    </form>
  {:else}
    <div class="r"><span class="n"><span class="t">Add a provider</span><span class="h">It stays off until you enable it.</span></span><Button text="Add a provider…" disabled={busy} onclick={() => (adding = true)} /></div>
  {/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
