<script lang="ts">
  // Step one: who you are and your passkey, in one step with up to four screens (the stepper does not grow): your name and username; your email (required on the hosted service, optional when self-hosted, where recovery then goes through the
  // server's admin); the eight-digit code sent to it, typed back (not a link); then the passkey, which makes the account and signs it in. The owner also pastes the setup code (filled from the setup link); an invited person's code is the
  // invitation's. Against fixtures (`demo`) the server is pretended: the email code is 00000000 and `?email_required=1` makes the address required.
  import { Button, Switch } from "@calternal/ui";
  import { emailConfirm, emailStart, setup, status, type AuthStatus } from "../../server/auth";
  import { say } from "../../server/say";
  import { cleanCode, emailCodeOk, emailLooksOk, usernameProblem } from "./account-rules";
  import type { Who } from "./flow";
  const fromLink = (() => { try { return new URLSearchParams(location.hash.slice(1)).get("setup") ?? new URLSearchParams(location.search).get("setup") ?? ""; } catch { return ""; } })();
  let { who, token: initial = fromLink, demo = false, coder = $bindable(false), ondone }: { who: Who; token?: string; demo?: boolean; coder?: boolean; ondone: () => void } = $props();
  // svelte-ignore state_referenced_locally
  let token = $state(initial), name = $state(""), username = $state(""), email = $state(""), code = $state(""), sub = $state<"who" | "email" | "code" | "passkey">("who");
  let challenge = $state(""), verification = $state<string | undefined>(), busy = $state(false), error = $state(""), touched = $state(false);
  let flags = $state<Pick<AuthStatus, "email_required" | "mail_available">>(demo ? { email_required: new URLSearchParams(location.search).get("email_required") === "1", mail_available: new URLSearchParams(location.search).get("mail") !== "0" } : {});
  $effect(() => { if (!demo) void status().then((s) => (flags = { email_required: !!s.email_required, mail_available: s.mail_available !== false }), () => {}); });
  const required = $derived(!!flags.email_required), canMail = $derived(flags.mail_available !== false);
  const userProblem = $derived(usernameProblem(username));
  const tokenFor = () => (token.trim() ? token : undefined);
  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
  async function sendCode() {
    busy = true; error = "";
    try {
      if (demo) { await wait(500); challenge = "demo"; } else challenge = (await emailStart(email, tokenFor())).challenge_id;
      code = ""; sub = "code";
    } catch (e) { error = required ? `${say(e, "send the code")} Verification is required to continue: send it again, or use a different address.` : say(e, "send the code"); }
    busy = false;
  }
  async function confirm() {
    busy = true; error = "";
    try {
      if (demo) { await wait(400); if (cleanCode(code) !== "00000000") throw new Error("wrong"); verification = "demo"; } else verification = (await emailConfirm(challenge, cleanCode(code), tokenFor())).verification_id;
      sub = "passkey";
    } catch (e) { error = e instanceof Error && e.message === "wrong" ? "That code is not right. Check the mail and try again." : say(e, "check the code"); }
    busy = false;
  }
  function afterEmail() {
    error = "";
    if (!email.trim()) { if (required) { error = "An email address is required here."; return; } sub = "passkey"; return; }
    if (!emailLooksOk(email)) { error = "That does not look like an email address."; return; }
    if (canMail) void sendCode(); else sub = "passkey";
  }
  async function create() {
    busy = true; error = "";
    try {
      if (demo) await wait(700); else await setup(token, name, { username, email: email.trim() || undefined, verification_id: verification });
      ondone();
    } catch (x) { error = say(x, who === "owner" ? "set up" : "join", { firstOwner: who === "owner" }); }
    busy = false;
  }
  const lead = $derived(who === "owner" ? "This installation has no owner yet." : "You were invited.");
</script>

<form onsubmit={(e) => { e.preventDefault(); if (sub === "who" && !userProblem && name.trim() && (demo || who !== "owner" || token.trim())) { sub = "email"; error = ""; } else if (sub === "email") afterEmail(); else if (sub === "code") void confirm(); else if (sub === "passkey") void create(); }}>
  {#if sub === "who"}
    <p class="note">{lead} Say who you are; the next screens ask for your email and make your passkey. There is no password.</p>
    {#if who === "owner" && !demo}<label>Setup code<input name="token" bind:value={token} autocomplete="off" spellcheck="false" required /></label>{/if}
    <label>Your name<input name="name" bind:value={name} autocomplete="name" required /></label>
    <label>Username<input name="username" bind:value={username} autocomplete="username" autocapitalize="none" spellcheck="false" oninput={() => (touched = true)} aria-invalid={touched && !!userProblem} /><small class:err={touched && !!userProblem}>{touched && userProblem ? userProblem : "3 to 32 letters, digits or hyphens. It is how others find you."}</small></label>
    <Switch label="I write code" hint="Adds three steps: your repositories, the command line, and direct mode on your own machine." bind:checked={coder} />
    <div class="go"><Button class="settings-action" type="submit" variant="primary" text="Continue" disabled={!name.trim() || !!userProblem || (who === "owner" && !demo && !token.trim())} /></div>
  {:else if sub === "email"}
    <p class="note">{required ? "An email address is required here. We send a code to it to check it is yours." : "An email address is optional. It lets you recover the account if you lose your passkey and your recovery codes."}</p>
    <label>Email{required ? "" : " (optional)"}<input name="email" type="email" bind:value={email} autocomplete="email" inputmode="email" autocapitalize="none" spellcheck="false" /></label>
    {#if !canMail}<p class="note">This server cannot send email, so an address cannot be verified here{email.trim() ? "; it is saved as given" : ""}. Recovery codes and a second passkey are the way back in.</p>
    {:else if !required && !email.trim()}<p class="note">Without an email, recovery goes through this server's administrator.</p>{/if}
    <div class="go"><Button class="settings-action" type="submit" variant="primary" text={email.trim() && canMail ? "Send the code" : "Continue"} disabled={busy || (required && !email.trim())} /><Button variant="quiet" text="Back" onclick={() => (sub = "who")} /></div>
  {:else if sub === "code"}
    <p class="note">We sent an eight-digit code to <strong>{email.trim()}</strong>. Type it here; it works for ten minutes.</p>
    <label>Code<input name="code" class="code" bind:value={code} inputmode="numeric" autocomplete="one-time-code" maxlength="9" spellcheck="false" /></label>
    <div class="go"><Button class="settings-action" type="submit" variant="primary" text="Confirm" disabled={busy || !emailCodeOk(cleanCode(code))} /></div>
    <div class="go"><Button variant="quiet" text="Send it again" disabled={busy} onclick={sendCode} /><Button variant="quiet" text="Use a different address" onclick={() => { sub = "email"; error = ""; }} /></div>
  {:else}
    <p class="note">Now make your passkey on this device. It is how you sign in; the account is created when you do.</p>
    <div class="go"><Button class="settings-action" type="submit" variant="primary" text="Create passkey" disabled={busy} /><Button variant="quiet" text="Back" onclick={() => (sub = "email")} /></div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</form>

<style>
  form { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  label { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
  small { font-size: var(--text-label); color: var(--ink-3); } small.err { color: var(--danger); }
  input { border: 0; min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); text-align: start; }
  .code { font-family: var(--font-mono); letter-spacing: 0.2em; font-size: var(--text-heading); }
  .error { font-size: var(--text-label); color: var(--danger); }
  .go { display: flex; gap: var(--s-4); flex-wrap: wrap; }
  .note strong { color: var(--ink); }
  @media (max-width: 520px) { .go :global(.settings-action) { width: 100%; } }
</style>
