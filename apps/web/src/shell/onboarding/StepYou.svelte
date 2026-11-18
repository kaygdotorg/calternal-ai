<script lang="ts">
  // Step one: who you are and your passkey. The owner pastes the setup code (it arrives in the setup link and is filled from it), an invited person's code is the invitation's; both give a name; then the browser makes the
  // passkey, which makes the account and signs it in. Against fixtures (`demo`) the passkey is only pretended, so the flow can be looked at and tested without a server.
  import { Button } from "@calternal/ui";
  import { setup } from "../../server/auth";
  import { say } from "../../server/say";
  import type { Who } from "./flow";
  const fromLink = (() => { try { return new URLSearchParams(location.hash.slice(1)).get("setup") ?? new URLSearchParams(location.search).get("setup") ?? ""; } catch { return ""; } })();
  let { who, token: initial = fromLink, demo = false, ondone }: { who: Who; token?: string; demo?: boolean; ondone: () => void } = $props();
  // svelte-ignore state_referenced_locally
  let token = $state(initial), name = $state(""), busy = $state(false), error = $state("");
  async function go(e: SubmitEvent) {
    e.preventDefault(); busy = true; error = "";
    try { if (demo) await new Promise((r) => setTimeout(r, 700)); else await setup(token, name); ondone(); }
    catch (x) { error = say(x, who === "owner" ? "set up" : "join"); }
    finally { busy = false; }
  }
</script>

<form onsubmit={go}>
  <p class="note">{who === "owner" ? "This installation has no owner yet. Say who you are and create a passkey on this device: it is how you sign in, there is no password." : "You were invited. Say who you are and create a passkey on this device; that makes your account."}</p>
  {#if who === "owner" && !demo}<label>Setup code<input name="token" bind:value={token} autocomplete="off" spellcheck="false" required /></label>{/if}
  <label>Your name<input name="name" bind:value={name} autocomplete="name" required /></label>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="go"><Button type="submit" variant="primary" text="Create passkey" disabled={busy || !name.trim() || (who === "owner" && !demo && !token.trim())} /></div>
</form>

<style>
  form { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  label { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
  input { border: 0; min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  .error { font-size: var(--text-label); color: var(--danger); }
  .go { display: flex; gap: var(--s-4); }
</style>
