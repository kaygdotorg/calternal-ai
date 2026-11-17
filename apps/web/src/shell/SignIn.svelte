<script lang="ts">
  // The signed-out screen, shown instead of the app when the real server is selected and this browser has no session. One card on the app's backdrop:
  // "Sign in with a passkey", or, on a fresh installation, the setup form (the setup link's token and a name). Success reloads into the app.
  import { Button, Card, Spinner } from "@calternal/ui";
  import { AuthError, setup, signIn } from "../server/auth";
  let { needsSetup }: { needsSetup: boolean } = $props();
  const fromLink = new URLSearchParams(location.hash.slice(1)).get("setup") ?? new URLSearchParams(location.search).get("setup") ?? "";
  let token = $state(fromLink);
  let name = $state("");
  let busy = $state(false);
  let error = $state("");
  const done = () => { const u = new URL(location.href); u.searchParams.delete("setup"); u.hash = ""; location.replace(u.pathname + u.search); };
  async function run(f: () => Promise<void>) {
    busy = true; error = "";
    try { await f(); done(); }
    catch (e) { error = e instanceof AuthError ? (e.code === "setup_token_invalid" || e.code === "invite_invalid" ? "That setup link is invalid or expired. Ask for a new one." : e.message) : "Something went wrong. Try again."; }
    finally { busy = false; }
  }
</script>

<main class="wrap">
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
</main>

<style>
  .wrap { min-height: 100dvh; display: grid; place-items: center; padding: var(--s-6); }
  .wrap :global(.card) { width: min(26rem, 100%); }
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  label { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
  input { min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  .error { font-size: var(--text-label); color: var(--danger, #c0392b); }
  .go { display: flex; align-items: center; gap: var(--s-4); }
</style>
