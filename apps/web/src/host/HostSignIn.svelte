<script lang="ts">
  /**
   * Passkey sign-in, first-owner setup, and one-time recovery codes display
   * for the declaration-driven host entry.
   * Mirrors `apps/web/src/shell/SignIn.svelte` on the shared backdrop and motion tokens.
   */
  import { Button, Callout, Card } from "@calternal/ui";
  import { setupHost, signInHost } from "./client";
  import type { SessionInfo } from "./types";

  let {
    needsSetup = false,
    pendingLogout = false,
    onsignedin,
  }: {
    needsSetup?: boolean;
    pendingLogout?: boolean;
    onsignedin: (session: SessionInfo) => void;
  } = $props();

  const fromLink =
    new URLSearchParams(location.hash.slice(1)).get("setup") ??
    new URLSearchParams(location.search).get("setup") ??
    "";

  let token = $state(fromLink);
  let name = $state("");
  let busy = $state(false);
  let error = $state("");
  let recovery = $state<{ session: SessionInfo; codes: string[] } | null>(null);
  let copied = $state(false);
  let confirmed = $state(false);

  async function submitSetup(e: Event) {
    e.preventDefault();
    if (!token.trim() || !name.trim() || busy) return;
    busy = true;
    error = "";
    try {
      const reply = await setupHost(token, name);
      if (reply.recovery_codes?.length) {
        recovery = { session: reply.session, codes: reply.recovery_codes };
      } else {
        onsignedin(reply.session);
      }
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not complete setup.";
    } finally {
      busy = false;
    }
  }

  async function submitSignIn(e: Event) {
    e.preventDefault();
    if (busy) return;
    busy = true;
    error = "";
    try {
      const session = await signInHost();
      onsignedin(session);
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not sign in.";
    } finally {
      busy = false;
    }
  }

  async function copyCodes() {
    if (!recovery) return;
    try {
      await navigator.clipboard.writeText(recovery.codes.join("\n"));
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      // ignore clipboard errors in headless environments
    }
  }

  function finishRecovery() {
    if (!recovery || !confirmed) return;
    const s = recovery.session;
    recovery = null;
    onsignedin(s);
  }
</script>

<main class="wrap" aria-label={recovery ? "Recovery codes" : needsSetup ? "Set up Calternal" : "Sign in"}>
  <p class="mark" aria-hidden="true">Calternal</p>

  {#if recovery}
    <Card label="Recovery codes" variant="solid" class="card">
      <div class="in step">
        <h1>Save your recovery codes</h1>
        <p class="note">
          These codes are shown once and are not stored on this device. Save them somewhere safe in case you lose access to your passkey.
        </p>
        <ul class="codes" aria-label="Recovery codes">
          {#each recovery.codes as code (code)}
            <li><code>{code}</code></li>
          {/each}
        </ul>
        <div class="code-actions">
          <Button
            type="button"
            variant="secondary"
            text={copied ? "Copied" : "Copy codes"}
            onclick={copyCodes}
          />
        </div>
        <label class="chk">
          <input type="checkbox" bind:checked={confirmed} />
          <span>I have saved these recovery codes</span>
        </label>
        <div class="go">
          <Button
            type="button"
            variant="primary"
            text="Continue"
            disabled={!confirmed}
            onclick={finishRecovery}
          />
        </div>
      </div>
    </Card>
  {:else if needsSetup}
    <Card label="Set up Calternal" variant="solid" class="card">
      <form class="in step" onsubmit={submitSetup}>
        <h1>Set up Calternal</h1>
        <p class="note">
          This host has no owner yet. Enter the one-time setup code, say who you are, and create a passkey on this device.
        </p>
        <label class="fld">
          <span>Setup code</span>
          <input
            name="token"
            type="text"
            bind:value={token}
            autocomplete="off"
            spellcheck="false"
            required
          />
        </label>
        <label class="fld">
          <span>Your name</span>
          <input
            name="name"
            type="text"
            bind:value={name}
            autocomplete="name"
            required
          />
        </label>
        <Callout message={error} />
        <div class="go">
          <Button
            type="submit"
            variant="primary"
            text="Create passkey"
            {busy}
            disabled={!token.trim() || !name.trim()}
          />
        </div>
      </form>
    </Card>
  {:else}
    <Card label="Sign in" variant="solid" class="card">
      <form class="in step" onsubmit={submitSignIn}>
        <h1>Sign in</h1>
        <p class="note">Use the passkey on this device, or a phone nearby.</p>
        {#if pendingLogout}
          <Callout message="Finishing your previous sign-out as soon as the host is reachable." />
        {/if}
        <Callout message={error} />
        <div class="go">
          <Button type="submit" variant="primary" text="Sign in with a passkey" {busy} />
        </div>
      </form>
    </Card>
  {/if}
</main>

<style>
  .wrap {
    min-height: 100dvh;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: var(--s-6);
    padding: var(--s-6);
    position: relative;
    z-index: 1;
  }
  .mark {
    font-family: var(--font-brand, var(--font-ui));
    font-size: var(--text-title);
    font-weight: var(--strong-weight);
    color: var(--ink);
    margin: 0;
    padding-inline: 0.3em;
  }
  .wrap :global(.card) {
    width: min(26rem, 92vw);
    animation: enter var(--dur-base) var(--ease-out);
  }
  .step {
    animation: stepIn var(--dur-base) var(--ease-out);
  }
  @keyframes enter {
    from { opacity: 0; scale: var(--enter-scale); }
  }
  @keyframes stepIn {
    from { opacity: 0; translate: 0 6px; }
  }
  @media (prefers-reduced-motion: reduce) {
    .wrap :global(.card), .step { animation: none; }
  }
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); }
  h1 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); color: var(--ink); }
  .note { margin: 0; font-size: var(--text-label); color: var(--ink-2); line-height: 1.4; }
  .fld { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-2); }
  .fld input {
    border: 0;
    min-height: var(--pill-h, 38px);
    padding: 0 var(--s-5);
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.07);
    color: var(--ink);
    font: inherit;
    font-size: var(--text-ui);
  }
  .fld input:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .codes {
    list-style: none;
    margin: 0;
    padding: var(--s-4);
    border-radius: var(--r-card);
    background: rgb(var(--ink-rgb) / 0.06);
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--s-2) var(--s-4);
  }
  .codes code {
    font-family: var(--font-mono);
    font-size: var(--text-label);
    color: var(--ink);
    letter-spacing: 0.03em;
  }
  .code-actions { display: flex; }
  .chk {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    font-size: var(--text-label);
    color: var(--ink);
    cursor: pointer;
  }
  .chk input { width: 18px; height: 18px; accent-color: var(--accent-base); }
  .go { display: flex; align-items: center; gap: var(--s-4); }
</style>
