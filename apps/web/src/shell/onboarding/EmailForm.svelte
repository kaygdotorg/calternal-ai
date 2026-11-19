<script lang="ts">
  // Adding or changing the address of a signed-in account: the address, then (when the server can send mail) the eight-digit code typed back, then done. No token is sent (the session proves who asks; the server may ask for a fresh passkey first).
  // On a server with no mail the address is only saved, unconfirmed. Used on the screen that asks a signed-in person for an address when the server's policy requires one (`email_setup_required`) and in Settings, Account.
  import { Button, Callout } from "@calternal/ui";
  import { emailConfirm, emailStart, fresh } from "../../server/auth";
  import { say } from "../../server/say";
  import { cleanCode, emailCodeOk, emailLooksOk } from "./account-rules";
  let { mail = true, initial = "", ondone, oncancel }: { mail?: boolean; initial?: string; ondone: (status: string) => void; oncancel?: () => void } = $props();
  // svelte-ignore state_referenced_locally
  let email = $state(initial), code = $state(""), challenge = $state(""), sub = $state<"email" | "code">("email"), busy = $state(false), error = $state("");
  async function send() {
    error = "";
    if (!emailLooksOk(email)) { error = "That does not look like an email address."; return; }
    busy = true;
    try {
      const r = (await fresh(() => emailStart(email))) as { challenge_id?: string; email_status?: string };
      if (r.challenge_id) { challenge = r.challenge_id; code = ""; sub = "code"; } else ondone(r.email_status ?? "unverified");
    } catch (e) { error = say(e, "send the code"); }
    busy = false;
  }
  async function confirm() {
    busy = true; error = "";
    try { const r = await emailConfirm(challenge, cleanCode(code)); ondone(r.email_status); } catch (e) { error = say(e, "check the code"); }
    busy = false;
  }
</script>

<form onsubmit={(e) => { e.preventDefault(); if (sub === "email") void send(); else void confirm(); }}>
  {#if sub === "email"}
    <label>Email<input name="email" type="email" bind:value={email} autocomplete="email" inputmode="email" autocapitalize="none" spellcheck="false" /></label>
    {#if !mail}<p class="note">This server cannot send email, so an address cannot be verified here. It is saved as given. Recovery codes and a second passkey are the way back in.</p>{/if}
    <Callout message={error} />
    <div class="go"><Button class="settings-action" type="submit" variant="primary" text={mail ? "Send the code" : "Save"} busy={busy} disabled={!email.trim()} />{#if oncancel}<Button variant="quiet" text="Cancel" onclick={oncancel} />{/if}</div>
  {:else}
    <p class="note">We sent an eight-digit code to <strong>{email.trim()}</strong>. Type it here; it works for ten minutes.</p>
    <label>Code<input name="code" class="code" bind:value={code} inputmode="numeric" autocomplete="one-time-code" maxlength="9" spellcheck="false" /></label>
    <Callout message={error} />
    <div class="go"><Button class="settings-action" type="submit" variant="primary" text="Confirm" busy={busy} disabled={!emailCodeOk(cleanCode(code))} /><Button variant="quiet" text="Send it again" disabled={busy} onclick={send} /><Button variant="quiet" text="Use a different address" onclick={() => (sub = "email")} /></div>
  {/if}
</form>

<style>
  form { display: grid; gap: var(--s-4); }
  .note { font-size: var(--text-label); color: var(--ink-3); } .note strong { color: var(--ink); }
  label { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
  input { border: 0; min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); text-align: start; }
  .code { font-family: var(--font-mono); letter-spacing: 0.2em; font-size: var(--text-heading); }
  .go { display: flex; gap: var(--s-4); flex-wrap: wrap; }
  @media (max-width: 520px) { .go :global(.settings-action) { width: 100%; } }
</style>
