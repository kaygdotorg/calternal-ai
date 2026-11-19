<script lang="ts">
  // Step two: the recovery codes, shown once, straight after the account is made. They come from the server's answer to setup (`takeSignupResult`) and are never kept: if this page was reloaded before they were saved, they cannot be shown
  // again, so the step offers a new set (the old one stops working). Against fixtures (`demo`) a pretended set is made.
  import { Button, Callout } from "@calternal/ui";
  import { regenerateRecoveryCodes, takeSignupResult } from "../../server/auth";
  import { say } from "../../server/say";
  import RecoveryCodes from "./RecoveryCodes.svelte";
  let { demo = false, ondone }: { demo?: boolean; ondone: () => void } = $props();
  const fake = () => Array.from({ length: 10 }, () => [...crypto.getRandomValues(new Uint8Array(16))].map((b) => b.toString(16).padStart(2, "0")).join(""));
  // svelte-ignore state_referenced_locally
  let codes = $state<string[] | null>(demo ? fake() : takeSignupResult()?.recovery_codes ?? null), busy = $state(false), error = $state("");
  async function again() { busy = true; error = ""; try { codes = demo ? fake() : await regenerateRecoveryCodes(); } catch (e) { error = say(e, "make new codes"); } busy = false; }
</script>

<div class="in">
  {#if codes}
    <p class="note">If you lose your passkey, one of these gets you back in. Save them now: they are not shown again.</p>
    <RecoveryCodes {codes} onsaved={ondone} />
  {:else}
    <p class="note">The recovery codes were shown once, and this page was reloaded before you saved them. They cannot be shown again. Make a new set (the old one stops working) and save it.</p>
    <Callout message={error} />
    <div class="go"><Button variant="primary" text="Make new codes" busy={busy} onclick={again} /></div>
  {/if}
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .go { display: flex; }
</style>
