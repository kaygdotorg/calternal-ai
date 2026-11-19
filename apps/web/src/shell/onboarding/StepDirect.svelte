<script lang="ts">
  // Coding step three: direct mode on your own machine. By default threads run sandboxed on a machine; direct runs them on the machine itself, with your own tools, files and credentials in reach. A switch for the machine,
  // `machines.set`; off is the safe default and the step can be skipped.
  import { Button, Callout, Switch } from "@calternal/ui";
  import { say } from "../../server/say";
  import type { MachineInfo, OnboardingApi } from "./api";
  let { api, ondone }: { api: OnboardingApi; ondone: () => void } = $props();
  let machine = $state<MachineInfo | null>(null), direct = $state(false), error = $state(""), loaded = $state(false);
  $effect(() => { void api.machineStatus().then((s) => { machine = s.machine; loaded = true; }, () => (loaded = true)); });
  async function set(v: boolean) {
    if (!machine) return;
    try { await api.setDirect(machine.id, v); direct = v; error = ""; } catch (e) { error = say(e, "change it"); }
  }
</script>

<div class="in">
  <p class="note">Threads run sandboxed by default: in a container, away from the rest of the machine. <strong>Direct</strong> runs them on the machine itself, with your own tools, files and credentials in reach, as if you typed the commands. Turn it on only for a machine that is yours.</p>
  {#if !loaded}
    <p class="note">Looking for your machine…</p>
  {:else if machine}
    <Switch label="Run threads directly on {machine.name}" hint={direct ? "On: threads can touch anything this user can." : "Off: sandboxed."} checked={direct} onchange={set} />
  {:else}
    <p class="note">There is no machine yet, so there is nothing to change. Add one in Settings, Machines.</p>
  {/if}
  <Callout message={error} />
  <div class="go"><Button variant="primary" text="Continue" onclick={ondone} /></div>
</div>

<style>
  .in { display: grid; gap: var(--s-5); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  .go { display: flex; gap: var(--s-4); }
</style>
