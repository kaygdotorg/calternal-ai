<script lang="ts">
  // Step three: a second passkey, so losing one device does not lock the account. Skipping is explicit (Not now, below) and the prompt returns in Settings, Account while there is only one passkey.
  import { Button, Callout, Tick } from "@calternal/ui";
  import { account } from "../../server/auth";
  import { say } from "../../server/say";
  let { demo = false, ondone }: { demo?: boolean; ondone: () => void } = $props();
  let busy = $state(false), error = $state(""), added = $state(false);
  async function add() {
    busy = true; error = "";
    try { if (demo) await new Promise((r) => setTimeout(r, 600)); else await account.addPasskey("Second passkey"); added = true; }
    catch (e) { error = say(e, "add a passkey"); }
    busy = false;
  }
</script>

<div class="in">
  {#if added}
    <p class="ready"><Tick checked animate size="var(--s-9)" /><span>Second passkey added.</span></p>
    <div class="go"><Button variant="primary" text="Continue" onclick={ondone} /></div>
  {:else}
    <p class="note">A second passkey, on another device or a hardware key, means losing one is not losing the account. It takes a few seconds.</p>
    <Callout message={error} />
    <div class="go"><Button class="settings-action" variant="primary" text="Add a passkey on another device" busy={busy} onclick={add} /></div>
  {/if}
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .ready { display: flex; align-items: center; gap: var(--s-5); }
  .go { display: flex; }
  @media (max-width: 520px) { .go :global(.settings-action) { width: 100%; } }
</style>
