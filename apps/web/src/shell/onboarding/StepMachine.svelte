<script lang="ts">
  // Step three: a machine, the computer the threads run on. If the instance already has one (set up when it was deployed) it shows as ready and there is nothing to do. Otherwise this makes the install script (single use, 15 minutes) and prints it
  // adds a machine; the screen looks again every two seconds and the step confirms the moment the machine is there (a count-up says how long it has waited; nothing is invented). `machines.onboarding` says whether one is ready; `machines.install` makes the script.
  import { Button, Callout, Tick } from "@calternal/ui";
  import { say } from "../../server/say";
  import type { Install, MachineStatus, OnboardingApi } from "./api";
  import CopyButton from "./CopyButton.svelte";
  import Waiting from "./Waiting.svelte";
  let { api, ondone, satisfied = $bindable(false) }: { api: OnboardingApi; ondone: () => void; /** True while a machine is connected: the step needs nothing more, so the card hides Skip and its warning. */ satisfied?: boolean } = $props();
  let status = $state<MachineStatus | null>(null), script = $state<Install | null>(null), error = $state(""), busy = $state(false);
  const online = $derived(status?.ready ? status.machine : null);
  $effect(() => { satisfied = !!online; return () => { satisfied = false; }; });
  async function look() { try { status = await api.machineStatus(); error = ""; } catch (e) { error = say(e, "look for machines"); status ??= { attached: false, ready: false, machine: null }; } }
  $effect(() => { void look(); const t = setInterval(() => void look(), 2000); return () => clearInterval(t); });
  async function make() { busy = true; try { script = await api.install(); error = ""; } catch (e) { error = say(e, "make the script"); } busy = false; }
</script>

<div class="in">
  <Callout message={error} />
  {#if status === null}
    <Waiting label="Looking for your machine" />
  {:else if online}
    <p class="ready"><Tick checked animate size="var(--s-9)" /><span><strong>{online.name}</strong><small>{status?.attached ? "Set up with this instance and connected" : "Connected"}{online.os ? `, ${online.os} ${online.arch}` : ""}. Threads can run here.</small></span></p>
    <div class="go"><Button class="settings-action" variant="primary" text="Continue" onclick={ondone} />{#if !script}<Button variant="quiet" text="Add another machine" disabled={busy} onclick={make} />{/if}</div>
    {#if script}
      <p class="note">Run this on the other machine. One use, valid for 15 minutes.</p>
      <div class="cmd"><code>{script.script}</code><CopyButton text={script.script} label="Copy the script" onfail={() => (error = "Could not copy. Select the script and copy it.")} /></div>
    {/if}
  {:else if status.attached}
    <p class="notice" role="status"><strong>This server's own machine has not connected yet.</strong> It starts with the server and usually connects within a minute. If it stays like this, the server could not reach its own public address: check the server's log, or add a machine of your own below and carry on.</p>
    <Waiting label="Waiting for the server's own machine" />
    {#if script}
      <p class="note">Run this on a machine of your own, as the user that should run the agents. One use, valid for 15 minutes.</p>
      <div class="cmd"><code>{script.script}</code><CopyButton text={script.script} label="Copy the script" onfail={() => (error = "Could not copy. Select the script and copy it.")} /></div>
    {:else}
      <div class="go"><Button variant="secondary" text="Add a machine of my own" disabled={busy} onclick={make} /></div>
    {/if}
  {:else}
    <p class="note">A machine is a computer your threads run on: a Mac or Linux box of your own, or a cloud server. Run this script on it, as the user that should run the agents:</p>
    {#if script}
      <div class="cmd"><code>{script.script}</code><CopyButton text={script.script} label="Copy the script" onfail={() => (error = "Could not copy. Select the script and copy it.")} /></div>
      <p class="note">One use, valid for 15 minutes.</p>
      <Waiting label="Waiting for the machine to connect" />
    {:else}
      <div class="go"><Button variant="primary" text="Show the script" disabled={busy} onclick={make} /></div>
    {/if}
  {/if}
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .ready { display: flex; align-items: center; gap: var(--s-5); }
  .ready span { display: grid; }
  .ready small { color: var(--ink-3); font-size: var(--text-label); }
  .cmd { display: flex; align-items: center; gap: var(--s-4); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); }
  code { flex: 1; min-width: 0; overflow-wrap: anywhere; font-family: var(--font-mono); font-size: var(--text-label); }
  .notice { font-size: var(--text-ui); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); }
  .notice strong { display: block; margin-bottom: var(--s-1); }
  .go { display: flex; gap: var(--s-4); }
</style>
