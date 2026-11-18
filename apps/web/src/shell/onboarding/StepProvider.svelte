<script lang="ts">
  // Step four: sign in to a provider (Claude Code, Codex) on the machine, from here. "Sign in" starts it on the machine (`provider.sign_in.start`); the machine answers with a link to open and, when the provider shows one back,
  // a code to type here (both come in the person's own request, `requests.list`); the screen follows the status until it is done. Nothing to run in a terminal. If that cannot be used (an older server, a machine that is
  // offline) the provider's own command is there to copy and Check again asks the machine (`agent.check`). One provider signed in ends the step.
  import { Button, Tick } from "@calternal/ui";
  import { say } from "../../server/say";
  import type { OnboardingApi, ProviderRow, SignInRequest, SignInStatus } from "./api";
  import CopyButton from "./CopyButton.svelte";
  import Waiting from "./Waiting.svelte";
  let { api, ondone }: { api: OnboardingApi; ondone: () => void } = $props();
  let rows = $state<ProviderRow[] | null>(null), error = $state(""), checking = $state<string | null>(null);
  /** The sign-in in progress, if any: which provider, what the machine asked for, how it is going. */
  let flow = $state<{ key: string; id: string; request: SignInRequest | null; status: SignInStatus; code: string; sending: boolean } | null>(null);
  const ready = $derived(rows?.some((r) => r.state === "signed_in") ?? false);
  async function look() { try { rows = await api.providers(); error = ""; } catch (e) { error = say(e, "read the providers"); rows ??= []; } }
  $effect(() => { void look(); const t = setInterval(() => void look(), 3000); return () => clearInterval(t); });
  // Follows a started sign-in: asks for its link and code until they exist, then for its status, until it is done or failed.
  $effect(() => {
    const f = flow;
    if (!f || f.status !== "pending") return;
    const t = setInterval(async () => {
      try {
        if (!flow || flow.id !== f.id) return;
        if (!flow.request) flow.request = await api.signInRequest(f.id);
        const st = await api.signInStatus(f.id);
        if (flow && flow.id === f.id) { flow.status = st; if (st === "done") void look(); }
      } catch (e) { error = say(e, "follow the sign-in"); }
    }, 1500);
    return () => clearInterval(t);
  });
  async function start(r: ProviderRow) {
    error = "";
    try { const s = await api.startSignIn(r.id, r.machineId); flow = { key: `${r.machineId}/${r.id}`, id: s.id, request: null, status: s.status, code: "", sending: false }; }
    catch (e) { error = say(e, "start the sign-in"); }
  }
  async function send() {
    if (!flow) return;
    flow.sending = true;
    try { flow.status = await api.answerSignIn(flow.id, flow.code); if (flow.status === "done") void look(); } catch (e) { error = say(e, "send the code"); }
    if (flow) flow.sending = false;
  }
  async function check(r: ProviderRow) { checking = `${r.machineId}/${r.id}`; try { await api.check(r.machineId, r.id); await look(); } catch (e) { error = say(e, "check"); } checking = null; }
  const STATE: Record<ProviderRow["state"], string> = { signed_in: "Signed in", not_signed_in: "Not signed in", not_installed: "Not installed on this machine", unknown: "Not checked yet", off: "Off in this space" };
</script>

<div class="in">
  {#if rows === null}
    <Waiting label="Asking the machine" />
  {:else if !rows.length}
    <p class="note">No machine is connected yet, so there is nothing to sign in to. Go back to add one, or skip and do it later.</p>
  {:else}
    <p class="note">The agents answer through a provider's own account. Sign in from here: the machine gives you a link to open.</p>
    <ul>
      {#each rows as r (r.machineId + r.id)}
        {@const mine = flow?.key === `${r.machineId}/${r.id}` ? flow : null}
        <li>
          <span class="t">{#if r.state === "signed_in"}<Tick checked animate={false} size="var(--s-7)" />{/if}<strong>{r.name}</strong><small>{r.machineName}: {STATE[r.state]}</small></span>
          {#if r.state !== "signed_in"}
            {#if mine && mine.status === "pending"}
              {#if !mine.request}
                <Waiting label="Waiting for the machine's link" />
              {:else}
                <ol class="how">
                  <li>{#if mine.request.url}<a href={mine.request.url} target="_blank" rel="noopener noreferrer">Open the {mine.request.label} sign-in page</a>{:else}Open the sign-in page on the machine.{/if}</li>
                  {#if mine.request.displayCode}<li>Confirm this code there: <code class="big">{mine.request.displayCode}</code></li>{/if}
                  {#if mine.request.needsCode}<li><label>Paste the code it gives you<input bind:value={mine.code} autocomplete="off" spellcheck="false" onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); void send(); } }} /></label> <Button variant="primary" text="Send the code" disabled={mine.sending || !mine.code.trim()} onclick={send} /></li>{:else}<li><Waiting label="Waiting for you to finish there" /></li>{/if}
                </ol>
              {/if}
            {:else}
              {#if mine?.status === "failed"}<p class="error" role="alert">That sign-in did not finish. Try again.</p>{/if}
              <Button variant="secondary" text={mine?.status === "failed" ? "Try again" : "Sign in"} onclick={() => start(r)} />
            {/if}
            {#if r.command && r.state === "not_signed_in"}
              <details><summary>Or sign in on the machine</summary>
                <span class="cmd"><code>{r.command}</code><CopyButton text={r.command} label="Copy the sign-in command" /></span>
                <Button variant="quiet" text="Check again" disabled={checking === `${r.machineId}/${r.id}`} onclick={() => check(r)} />
              </details>
            {:else}<Button variant="quiet" text="Check again" disabled={checking === `${r.machineId}/${r.id}`} onclick={() => check(r)} />{/if}
          {/if}
        </li>
      {/each}
    </ul>
    {#if checking}<Waiting label="Checking on the machine" />{/if}
    {#if ready}<div class="go"><Button variant="primary" text="Continue" onclick={ondone} /></div>{/if}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  ul { display: grid; gap: var(--s-4); list-style: none; padding: 0; margin: 0; }
  ul > li { display: grid; gap: var(--s-3); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.05); }
  .t { display: flex; align-items: center; gap: var(--s-4); flex-wrap: wrap; }
  .t small { color: var(--ink-3); font-size: var(--text-label); }
  .how { display: grid; gap: var(--s-3); padding-inline-start: var(--s-7); margin: 0; font-size: var(--text-label); }
  .how li { display: block; padding: 0; background: none; }
  .how a { color: var(--accent); text-decoration: underline; }
  .big { font-family: var(--font-mono); font-size: var(--text-heading); letter-spacing: 0.08em; }
  label { display: grid; gap: var(--s-2); color: var(--ink-3); }
  input { border: 0; min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  summary { cursor: pointer; font-size: var(--text-label); color: var(--ink-3); }
  details { display: grid; gap: var(--s-3); }
  .cmd { display: flex; align-items: center; gap: var(--s-4); }
  code { flex: 1; min-width: 0; overflow-wrap: anywhere; font-family: var(--font-mono); font-size: var(--text-label); }
  .error { font-size: var(--text-label); color: var(--danger); }
  .go { display: flex; gap: var(--s-4); }
</style>
