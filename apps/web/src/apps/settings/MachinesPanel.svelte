<script lang="ts">
  // The Machines page: the machines a person's threads can run on, each with where its threads run (sandboxed or direct), and the guided way to add one (by SSH; the server never keeps the key).
  // Fixture machines and values; the real list is the server's (`machines.list`).
  import { Button, Selector, Sheet } from "@calternal/ui";
  import { values } from "./store.svelte";
  const MACHINES: { id: string; name: string; state: string }[] = [
    { id: "calternal-dev-02-std", name: "calternal-dev-02-std", state: "Online" },
    { id: "calternal-dev-01-compute02", name: "calternal-dev-01-compute02", state: "Online" },
    { id: "kayg-laptop", name: "kayg-laptop", state: "Offline" },
  ];
  let added = $state<string[]>(values.get<string[]>("machines.added", []));
  const all = $derived([...MACHINES, ...added.map((n) => ({ id: n, name: n, state: "Online" }))]);
  const mode = (id: string) => values.get<string>(`machines.${id}.mode`, "sandboxed");
  // The guided flow: how, then where, then confirm.
  let open = $state(false), step = $state(0), host = $state(""), user = $state("");
  const done = () => { if (host.trim()) { added = [...added, host.trim()]; values.set("machines.added", added); } open = false; step = 0; host = ""; user = ""; };
</script>

<ul class="ms">
  {#each all as m (m.id)}
    <li data-setting="machines.{m.id}">
      <span class="n"><span class="t">{m.name}</span><span class="h">{m.state}</span></span>
      <Selector class="sel-inline" label="Where threads run on {m.name}" options={[{ value: "sandboxed", label: "Sandboxed" }, { value: "direct", label: "Direct" }]} value={mode(m.id)} onchange={(v) => values.set(`machines.${m.id}.mode`, v)} />
    </li>
  {/each}
</ul>
<div class="add"><span class="l"><span class="t">Add a machine</span><span class="h">By SSH. The server never keeps your key.</span></span><Button class="settings-action" text="Add a machine…" variant="secondary" onclick={() => (open = true)} /></div>

<Sheet {open} label="Add a machine" onclose={() => { open = false; step = 0; }} side={typeof matchMedia !== "undefined" && matchMedia("(max-width: 759px)").matches ? "bottom" : "center"}>
  <div class="flow">
    <h1>Add a machine</h1>
    {#if step === 0}
      <p class="h">Step 1 of 3. How will it connect?</p>
      <Selector class="row-selector" label="Connection" options={[{ value: "ssh", label: "SSH" }]} value="ssh" onchange={() => {}} />
      <div class="f"><Button text="Continue" variant="primary" onclick={() => (step = 1)} /></div>
    {:else if step === 1}
      <p class="h">Step 2 of 3. Where is it?</p>
      <label class="fl"><span>Host name or address</span><input class="field" bind:value={host} autocomplete="off" spellcheck="false" placeholder="build-01.example.org" /></label>
      <label class="fl"><span>User</span><input class="field" bind:value={user} autocomplete="off" spellcheck="false" placeholder="ubuntu" /></label>
      <div class="f"><Button text="Back" variant="quiet" onclick={() => (step = 0)} /><Button text="Continue" variant="primary" disabled={!host.trim()} onclick={() => (step = 2)} /></div>
    {:else}
      <p class="h">Step 3 of 3. Check and add.</p>
      <p>The server will connect to <strong>{user ? user + "@" : ""}{host}</strong> with a key you authorize on that machine; it does not keep your own key. This is a fixture: nothing connects.</p>
      <div class="f"><Button text="Back" variant="quiet" onclick={() => (step = 1)} /><Button text="Add the machine" variant="primary" onclick={done} /></div>
    {/if}
  </div>
</Sheet>

<style>
  .ms { display: grid; }
  li { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--s-6); min-height: var(--row-h-settings); padding: var(--s-3) var(--s-6); }
  .n, .l { display: grid; min-width: 0; gap: var(--s-1); }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; }
  .h { font-size: var(--text-label); color: var(--ink-3); }
  .add { display: flex; align-items: center; justify-content: space-between; gap: var(--s-6); padding: var(--s-3) var(--s-6); border-top: 1px solid var(--line); }
  .flow { display: grid; gap: var(--s-5); padding: var(--s-6); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .fl { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
  .field { min-height: var(--pill-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  .f { display: flex; justify-content: flex-end; gap: var(--s-3); }
  @media (max-width: 600px) { li, .add { flex-direction: column; align-items: stretch; } }
</style>
