<script lang="ts">
  // Step four: sign in to a provider (Claude Code, Codex) on the machine. Each provider shows what the machine told us: signed in, not signed in (with the provider's own sign-in command to copy and run there), or not installed. A check
  // is asked for and the screen follows the answer. When one is signed in the step is done. (The relayed sign-in, a link to open here instead of a command to run there, is the backend lane #48's.)
  import { Button, Tick } from "@calternal/ui";
  import { say } from "../../server/say";
  import type { OnboardingApi, ProviderRow } from "./api";
  import CopyButton from "./CopyButton.svelte";
  import Waiting from "./Waiting.svelte";
  let { api, ondone }: { api: OnboardingApi; ondone: () => void } = $props();
  let rows = $state<ProviderRow[] | null>(null), error = $state(""), checking = $state<string | null>(null);
  const ready = $derived(rows?.some((r) => r.state === "signed_in") ?? false);
  async function look() { try { rows = await api.providers(); error = ""; } catch (e) { error = say(e, "read the providers"); rows ??= []; } }
  $effect(() => { void look(); const t = setInterval(() => void look(), 3000); return () => clearInterval(t); });
  async function check(r: ProviderRow) { checking = `${r.machineId}/${r.id}`; try { await api.check(r.machineId, r.id); await look(); } catch (e) { error = say(e, "check"); } checking = null; }
  const STATE: Record<ProviderRow["state"], string> = { signed_in: "Signed in", not_signed_in: "Not signed in", not_installed: "Not installed on this machine", unknown: "Not checked yet", off: "Off in this space" };
</script>

<div class="in">
  {#if rows === null}
    <Waiting label="Asking the machine" />
  {:else if !rows.length}
    <p class="note">No machine is connected yet, so there is nothing to sign in to. Go back to add one, or skip and do it later.</p>
  {:else}
    <p class="note">The agents answer through a provider's own account. Sign in on the machine with the command shown, then check.</p>
    <ul>
      {#each rows as r (r.machineId + r.id)}
        <li>
          <span class="t">{#if r.state === "signed_in"}<Tick checked animate={false} size="var(--s-7)" />{/if}<strong>{r.name}</strong><small>{r.machineName}: {STATE[r.state]}</small></span>
          {#if r.state !== "signed_in"}
            {#if r.command && r.state === "not_signed_in"}<span class="cmd"><code>{r.command}</code><CopyButton text={r.command} label="Copy the sign-in command" /></span>{/if}
            <Button variant="quiet" text="Check again" disabled={checking === `${r.machineId}/${r.id}`} onclick={() => check(r)} />
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
  li { display: grid; gap: var(--s-3); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.05); }
  .t { display: flex; align-items: center; gap: var(--s-4); flex-wrap: wrap; }
  .t small { color: var(--ink-3); font-size: var(--text-label); }
  .cmd { display: flex; align-items: center; gap: var(--s-4); }
  code { flex: 1; min-width: 0; overflow-wrap: anywhere; font-family: var(--font-mono); font-size: var(--text-label); }
  .error { font-size: var(--text-label); color: var(--danger); }
  .go { display: flex; gap: var(--s-4); }
</style>
