<script lang="ts">
  // "New space": the flow behind the space menu. A space is one kind: it has a name, members (people from the groups you belong to, invited; Personal alone takes none) and the Agent access question, which is asked here, once, when the space is made. Fixture only.
  import { toast } from "@calternal/ui";
  import GlassSheet from "./GlassSheet.svelte";
  import { GROUPS } from "../data/apps";
  import { Switch } from "@calternal/ui";
  import { share } from "../share.svelte";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  // Agent access is ONE switch, asked when the space is made (on by default). What an agent may see is not chosen here: the scope is chosen per run, when an agent is started.
  let name = $state(""), invited = $state<string[]>([]), agents = $state(true);
  const people = $derived([...new Set(GROUPS.filter((g) => invited.includes(g.id)).flatMap((g) => g.people))]);
  function make() { const s = share.create(name, people.length + 1, agents); onclose(); toast(`Space "${s.name}" created`); name = ""; invited = []; agents = true; }
  const toggle = (id: string) => (invited = invited.includes(id) ? invited.filter((x) => x !== id) : [...invited, id]);
</script>

<GlassSheet {open} label="New space" {onclose}>
  <form class="ns" data-new-space onsubmit={(e) => { e.preventDefault(); make(); }}>
    <h2>New space</h2>
    <label class="f"><span>Name</span><input bind:value={name} placeholder="Book club, Studio, Home…" aria-label="Space name" /></label>
    <fieldset><legend>Invite people from your groups</legend>
      {#each GROUPS as g (g.id)}<label class="g"><input type="checkbox" checked={invited.includes(g.id)} onchange={() => toggle(g.id)} /><span><b>{g.name}</b><small>{g.people.join(", ")}</small></span></label>{/each}
      <p class="h">They get a link that expires. {people.length ? `${people.length} will be invited.` : "You can invite people later; a space can also be just yours."}</p>
    </fieldset>
    <fieldset><legend>Agent access</legend>
      <Switch label="Let agents work in this space" hint="What an agent may see is chosen each time an agent is started." checked={agents} onchange={(v: boolean) => (agents = v)} />
    </fieldset>
    <div class="bt"><button type="button" onclick={onclose}>Cancel</button><button type="submit" class="pri">Create space</button></div>
  </form>
</GlassSheet>

<style>
  .ns { display: grid; gap: var(--s-5); padding: var(--s-7); min-width: min(34rem, 88vw); box-sizing: border-box; } h2 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .f { display: grid; gap: var(--s-2); color: var(--ink-2); font-size: var(--text-label); } .f input { min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); border: 0; color: var(--ink); font: inherit; font-size: var(--text-ui); outline: 0; } .f input:focus-visible { box-shadow: 0 0 0 2px var(--accent); }
  fieldset { border: 0; margin: 0; padding: 0; display: grid; gap: var(--s-3); } legend { padding: 0; margin-bottom: var(--gap-head-items); font-weight: var(--strong-weight); color: var(--ink); font-size: var(--text-ui); }
  .g { display: flex; align-items: flex-start; gap: var(--s-4); padding: var(--s-3) var(--s-4); border-radius: var(--r-select); min-height: var(--row-h); } .g span { display: grid; gap: 1px; } .g b { font-weight: var(--ui-weight); color: var(--ink); font-size: var(--text-ui); } .g small, .h { color: var(--ink-2); font-size: var(--text-label); line-height: 1.35; } .h { margin: 0; } .g input { margin-top: 4px; accent-color: var(--accent); }
  .bt { display: flex; justify-content: flex-end; gap: var(--s-4); } button { min-height: var(--row-h); padding: 0 var(--s-6); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font-size: var(--text-ui); } button.pri { background: var(--badge-bg); color: #fff; }
</style>
