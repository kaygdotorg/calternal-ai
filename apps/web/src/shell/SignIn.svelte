<script lang="ts">
  // The signed-out screen, shown instead of the app when the real server is selected and this browser has no session. One card on the app's backdrop:
  // "Sign in with a passkey", or, on a fresh installation, the setup form (the setup link's token and a name). Success reloads into the app.
  import { Button, Card, Spinner } from "@calternal/ui";
  import { setup, signIn } from "../server/auth";
  import { say } from "../server/say";
  let { needsSetup = false, version, offline = false }: { needsSetup?: boolean; version?: { server: number; expected: number }; offline?: boolean } = $props();
  const fromLink = new URLSearchParams(location.hash.slice(1)).get("setup") ?? new URLSearchParams(location.search).get("setup") ?? "";
  let token = $state(fromLink);
  let name = $state("");
  let busy = $state(false);
  let error = $state("");
  const done = () => { const u = new URL(location.href); u.searchParams.delete("setup"); u.hash = ""; location.replace(u.pathname + u.search); };
  async function run(f: () => Promise<void>) {
    busy = true; error = "";
    try { await f(); done(); }
    catch (e) { error = say(e, needsSetup ? "set up" : "sign in"); }
    finally { busy = false; }
  }
</script>

<main class="wrap">
  <p class="mark" aria-hidden="true">Calternal</p>
{#if offline}
  <Card label="Server unreachable" variant="solid" class="card">
    <div class="in">
      <h1>Can't reach the server</h1>
      <p class="note">Calternal could not get an answer. Check your connection, and that the server is running, then try again.</p>
      <div class="go"><Button variant="primary" text="Try again" onclick={() => location.reload()} /></div>
    </div>
  </Card>
{:else if version}
  <Card label="Version mismatch" variant="solid" class="card">
    <div class="in">
      <h1>{version.server > version.expected ? "This page is out of date" : "The server is out of date"}</h1>
      <p class="note">{version.server > version.expected ? "The server is newer than this page and may answer in ways it does not understand. Reload to get the current page." : "This page is newer than the server and may ask for things it does not have yet. Ask whoever runs the server to update it."} (Server protocol {version.server}, this page {version.expected}.)</p>
      <div class="go"><Button variant="primary" text="Reload" onclick={() => location.reload()} /></div>
    </div>
  </Card>
{:else}
  <Card label={needsSetup ? "Set up Calternal" : "Sign in"} variant="solid" class="card">
    <form class="in" onsubmit={(e) => { e.preventDefault(); void run(needsSetup ? () => setup(token, name) : signIn); }}>
      <h1>{needsSetup ? "Set up Calternal" : "Sign in"}</h1>
      {#if needsSetup}
        <p class="note">This installation has no owner yet. Paste the setup link's code, say who you are, and create a passkey on this device.</p>
        <label>Setup code<input name="token" bind:value={token} autocomplete="off" spellcheck="false" required /></label>
        <label>Your name<input name="name" bind:value={name} autocomplete="name" required /></label>
      {:else}
        <p class="note">Use the passkey on this device, or a phone nearby.</p>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="go">
        <Button type="submit" variant="primary" text={needsSetup ? "Create passkey" : "Sign in with a passkey"} disabled={busy || (needsSetup && (!token.trim() || !name.trim()))} />
        {#if busy}<Spinner />{/if}
      </div>
    </form>
  </Card>
{/if}
</main>

<style>
  .wrap { min-height: 100dvh; display: grid; place-content: center; justify-items: center; gap: var(--s-6); padding: var(--s-6); background: linear-gradient(rgb(var(--tint, 247 247 247) / 0.55) 0 0), var(--wall-cheap, none) center / cover no-repeat; }
  .mark { font-family: var(--font-brand, inherit); font-size: var(--text-title); font-weight: var(--strong-weight); color: var(--ink); }
  .wrap :global(.card) { width: min(26rem, 100%); }
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  label { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
  input { border: 0; min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  .error { font-size: var(--text-label); color: var(--danger, #c0392b); }
  .go { display: flex; align-items: center; gap: var(--s-4); }
</style>
