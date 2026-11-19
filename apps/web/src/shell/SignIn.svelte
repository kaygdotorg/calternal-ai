<script lang="ts">
  // The signed-out screen, shown instead of the app when the real server is selected and this browser has no session. One card on the app's backdrop:
  // "Sign in with a passkey", or, on a fresh installation, the setup form (the setup link's token and a name). Success reloads into the app.
  import { Button, Callout, Card } from "@calternal/ui";
  import { setup, signIn } from "../server/auth";
  import { say } from "../server/say";
  import { finishPending, providers, returning, start, type Provider } from "../server/oidc";
  let { needsSetup = false, invite, version, offline = false }: { needsSetup?: boolean; invite?: string; version?: { server: number; expected: number }; offline?: boolean } = $props();
  const joining = $derived(!!invite && !needsSetup);
  const fromLink = new URLSearchParams(location.hash.slice(1)).get("setup") ?? new URLSearchParams(location.search).get("setup") ?? "";
  let token = $state(fromLink);
  $effect(() => { if (invite) token = invite; });
  let name = $state("");
  let busy = $state(false);
  let error = $state("");
  const done = () => { const u = new URL(location.href); u.searchParams.delete("setup"); u.hash = ""; location.replace(joining ? import.meta.env.BASE_URL : u.pathname + u.search); };
  let outside = $state<Provider[]>([]);
  let finishing = $state(returning());
  $effect(() => {
    if (needsSetup || joining || offline || version) return;
    if (finishing) { void run(async () => { await finishPending(); }).finally(() => (finishing = false)); return; }
    void providers().then((l) => (outside = l));
  });
  const via = async (p: Provider) => { busy = true; error = ""; try { await start(p.id); } catch (e) { error = say(e, "sign in"); busy = false; } };
  async function run(f: () => Promise<void>) {
    busy = true; error = "";
    try { await f(); done(); }
    catch (e) { error = say(e, needsSetup ? "set up" : joining ? "join" : "sign in", { firstOwner: needsSetup }); }
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
  <Card label={needsSetup ? "Set up Calternal" : joining ? "Join Calternal" : "Sign in"} variant="solid" class="card">
    <form class="in" onsubmit={(e) => { e.preventDefault(); void run(needsSetup || joining ? () => setup(token, name) : signIn); }}>
      <h1>{needsSetup ? "Set up Calternal" : joining ? "Join Calternal" : "Sign in"}</h1>
      <!-- The providers an admin has enabled (generic only): a button each, above the passkey button; either one signs you in. Nothing is built for them until the server offers them. -->
      <div class="providers" data-slot="providers">
        {#each outside as p (p.id)}
          <Button text={`Continue with ${p.label}`} variant="secondary" disabled={busy || finishing} onclick={() => via(p)} />
        {/each}
        {#if outside.length}<p class="note or">or</p>{/if}
      </div>
      {#if joining}
        <p class="note">You were invited. Say who you are and create a passkey on this device; that makes your account. If you already have one, sign in first, then open the link again.</p>
        <label>Your name<input name="name" bind:value={name} autocomplete="name" required /></label>
        <p class="note"><button type="button" class="link" onclick={() => (location.href = import.meta.env.BASE_URL)}>I already have an account</button></p>
      {:else if needsSetup}
        <p class="note">This installation has no owner yet. Paste the setup link's code, say who you are, and create a passkey on this device.</p>
        <label>Setup code<input name="token" bind:value={token} autocomplete="off" spellcheck="false" required /></label>
        <label>Your name<input name="name" bind:value={name} autocomplete="name" required /></label>
      {:else}
        <p class="note">Use the passkey on this device, or a phone nearby.</p>
      {/if}
      <Callout message={error} />
      <div class="go">
        <Button type="submit" variant="primary" text={needsSetup ? "Create passkey" : joining ? "Create passkey and join" : "Sign in with a passkey"} busy={busy} disabled={(needsSetup || joining) && (!token.trim() || !name.trim())} />
      </div>
    </form>
  </Card>
{/if}
</main>

<style>
  .providers:empty { display: none; }
  .providers { display: flex; flex-direction: column; gap: var(--s-2); }
  .providers :global(button) { width: 100%; }
  .or { text-align: center; margin: 0; }
  .wrap { min-height: 100dvh; display: grid; place-content: center; justify-items: center; gap: var(--s-6); padding: var(--s-6); background: linear-gradient(rgb(var(--tint) / 0.55) 0 0), var(--wall-cheap, none) center / cover no-repeat; }
  /* The script face's last "l" swings past its advance width; the box is padded so the whole stroke is inside it. Without the padding Safari paints the overhang as a separate piece while the word moves (a stray comma-like mark after the wordmark when the page re-centres). */
  .mark { font-family: var(--font-brand, inherit); font-size: var(--text-title); font-weight: var(--strong-weight); color: var(--ink); padding-inline: 0.3em; }
  .wrap :global(.card) { width: min(26rem, 100%); }
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  label { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
  input { border: 0; min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  .link { text-decoration: underline; color: var(--ink-2); font-size: var(--text-label); }
  .go { display: flex; align-items: center; gap: var(--s-4); }
</style>
