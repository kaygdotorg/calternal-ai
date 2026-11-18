<script lang="ts">
  // Step three: a machine, the computer the threads run on. If the instance already has one (set up when it was deployed) it shows as ready and there is nothing to do. Otherwise this makes a one-use code and prints the command that
  // connects a machine with it; the screen looks again every two seconds and the step confirms the moment the machine is there (a count-up says how long it has waited; nothing is invented).
  import { Button, Tick } from "@calternal/ui";
  import { say } from "../../server/say";
  import type { EnrolCode, MachineInfo, OnboardingApi } from "./api";
  import CopyButton from "./CopyButton.svelte";
  import Waiting from "./Waiting.svelte";
  let { api, ondone }: { api: OnboardingApi; ondone: () => void } = $props();
  let machines = $state<MachineInfo[] | null>(null), code = $state<EnrolCode | null>(null), error = $state(""), busy = $state(false);
  const online = $derived(machines?.find((m) => m.online) ?? null);
  async function look() { try { machines = await api.machines(); error = ""; } catch (e) { error = say(e, "look for machines"); machines ??= []; } }
  $effect(() => { void look(); const t = setInterval(() => void look(), 2000); return () => clearInterval(t); });
  async function make() { busy = true; try { code = await api.makeCode(); error = ""; } catch (e) { error = say(e, "make a code"); } busy = false; }
</script>

<div class="in">
  {#if machines === null}
    <Waiting label="Looking for your machine" />
  {:else if online}
    <p class="ready"><Tick checked animate size="var(--s-9)" /><span><strong>{online.name}</strong><small>Connected, {online.os} {online.arch}. Threads can run here.</small></span></p>
    <div class="go"><Button variant="primary" text="Continue" onclick={ondone} /></div>
  {:else}
    <p class="note">A machine is a computer your threads run on: a Mac or Linux box of your own, or a cloud server. Run this on it, as the user that should run the agents:</p>
    {#if code}
      <div class="cmd"><code>{code.command}</code><CopyButton text={code.command} onfail={() => (error = "Could not copy. Select the command and copy it.")} /></div>
      <p class="note">One use, valid for 15 minutes.</p>
      <Waiting label="Waiting for the machine to connect" />
    {:else}
      <div class="go"><Button variant="primary" text="Show the command" disabled={busy} onclick={make} /></div>
    {/if}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .ready { display: flex; align-items: center; gap: var(--s-5); }
  .ready span { display: grid; }
  .ready small { color: var(--ink-3); font-size: var(--text-label); }
  .cmd { display: flex; align-items: center; gap: var(--s-4); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); }
  code { flex: 1; min-width: 0; overflow-wrap: anywhere; font-family: var(--font-mono); font-size: var(--text-label); }
  .error { font-size: var(--text-label); color: var(--danger); }
  .go { display: flex; gap: var(--s-4); }
</style>
