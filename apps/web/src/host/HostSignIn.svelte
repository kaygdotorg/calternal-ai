<script lang="ts">
  /**
   * Signed-out screen for the declaration-driven host entry.
   * Mirrors `apps/web/src/shell/SignIn.svelte` on the shared backdrop and tokens.
   */
  import { Button, Callout, Card } from "@calternal/ui";
  import { signInHost } from "./client";
  import type { SessionInfo } from "./types";

  let { onsignedin }: { onsignedin: (session: SessionInfo) => void } = $props();
  let name = $state("Ada");
  let busy = $state(false);
  let error = $state("");

  async function submit(e: Event) {
    e.preventDefault();
    if (!name.trim() || busy) return;
    busy = true;
    error = "";
    try {
      const session = await signInHost(name);
      onsignedin(session);
    } catch (err) {
      error = err instanceof Error ? err.message : "Could not sign in.";
    } finally {
      busy = false;
    }
  }
</script>

<main class="wrap" aria-label="Sign in">
  <p class="mark" aria-hidden="true">Calternal</p>
  <Card label="Sign in" variant="solid" class="card">
    <form class="in" onsubmit={submit}>
      <h1>Sign in</h1>
      <p class="note">Enter your name to open your personal space on the host.</p>
      <label>
        <span>Your name</span>
        <input
          name="name"
          type="text"
          bind:value={name}
          autocomplete="name"
          placeholder="Your name"
          required
        />
      </label>
      <Callout message={error} />
      <div class="go">
        <Button type="submit" variant="primary" text="Sign in" {busy} disabled={!name.trim()} />
      </div>
    </form>
  </Card>
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
  @keyframes enter {
    from { opacity: 0; scale: var(--enter-scale); }
  }
  @media (prefers-reduced-motion: reduce) {
    .wrap :global(.card) { animation: none; }
  }
  .in { display: grid; gap: var(--s-5); padding: var(--s-7); }
  h1 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); color: var(--ink); }
  .note { margin: 0; font-size: var(--text-label); color: var(--ink-3); }
  label { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-2); }
  input {
    border: 0;
    min-height: var(--pill-h, 38px);
    padding: 0 var(--s-5);
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.07);
    color: var(--ink);
    font: inherit;
    font-size: var(--text-ui);
  }
  input:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .go { display: flex; align-items: center; gap: var(--s-4); }
</style>
