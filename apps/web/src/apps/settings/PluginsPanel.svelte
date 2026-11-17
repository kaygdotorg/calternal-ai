<script lang="ts">
  // The Plugins page: installed plugins (a switch, what each may reach, and its own options opening from its row) and the ones available to install. Fixture catalogue; the real one is the
  // server's (`plugins.list`). A plugin's own settings are declared by the plugin; the options here stand in for them.
  import { Button, Icon, Switch } from "@calternal/ui";
  import { values } from "./store.svelte";
  interface Plugin { id: string; name: string; access: string; options: { id: string; label: string }[] }
  const CATALOGUE: Plugin[] = [
    { id: "calternal-threads", name: "Threads", access: "Reads and writes threads in your spaces; starts agent turns", options: [{ id: "show-steps", label: "Show the steps of a turn" }, { id: "confirm-stop", label: "Ask before stopping a turn" }] },
    { id: "calternal-notes", name: "Notes", access: "Reads and writes notes in your spaces", options: [{ id: "autosave", label: "Save as you type" }] },
    { id: "calternal-settings", name: "Settings", access: "Reads and writes your settings", options: [] },
    { id: "calternal-calendar", name: "Calendar", access: "Reads and writes calendars you connect", options: [{ id: "week-start", label: "Weeks start on Monday" }] },
    { id: "calternal-contacts", name: "Contacts", access: "Reads and writes contacts in your spaces", options: [] },
  ];
  let installed = $state<string[]>(values.get<string[]>("plugins.installed", ["calternal-threads", "calternal-notes", "calternal-settings"]));
  const save = () => values.set("plugins.installed", installed);
  let openId = $state<string | null>(null);
  const on = (id: string) => values.get<boolean>(`plugins.${id}.enabled`, true);
</script>

<h3 class="gh">Installed</h3>
<ul>
  {#each CATALOGUE.filter((p) => installed.includes(p.id)) as p (p.id)}
    <li data-setting="plugins.{p.id}">
      <div class="r">
        <span class="sw"><Switch label={p.name} hint="May: {p.access}" checked={on(p.id)} onchange={(v) => values.set(`plugins.${p.id}.enabled`, v)} /></span>
        {#if p.options.length}<button type="button" class="drill" aria-expanded={openId === p.id} aria-label="{p.name} options" onclick={() => (openId = openId === p.id ? null : p.id)}><span class="h">Options</span><Icon inline name="chevron" /></button>{/if}
      </div>
      {#if openId === p.id}
        <div class="opts">{#each p.options as o (o.id)}<Switch label={o.label} checked={values.get<boolean>(`plugins.${p.id}.${o.id}`, true)} onchange={(v) => values.set(`plugins.${p.id}.${o.id}`, v)} />{/each}</div>
      {/if}
    </li>
  {/each}
</ul>
<h3 class="gh">Available</h3>
<ul>
  {#each CATALOGUE.filter((p) => !installed.includes(p.id)) as p (p.id)}
    <li data-setting="plugins.{p.id}"><div class="r"><span class="n"><span class="t">{p.name}</span><span class="h">Would be able to: {p.access.charAt(0).toLowerCase() + p.access.slice(1)}</span></span><Button text="Install" variant="secondary" onclick={() => { installed = [...installed, p.id]; save(); }} /></div></li>
  {:else}
    <li><div class="r"><span class="h">Everything available is installed.</span></div></li>
  {/each}
</ul>

<style>
  .gh { padding: var(--s-4) var(--s-6) 0; font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  ul { display: grid; }
  .r { display: flex; align-items: center; justify-content: space-between; gap: var(--s-5); min-height: var(--row-h-settings); padding: var(--s-3) var(--s-6); }
  .n { display: grid; flex: 1; min-width: 0; gap: var(--s-1); }
  .sw { flex: 1; min-width: 0; }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .h { font-size: var(--text-label); color: var(--ink-3); }
  .drill { display: inline-flex; align-items: center; gap: var(--s-2); }
  .drill[aria-expanded="true"] :global(svg) { rotate: 90deg; }
  .opts { display: grid; padding: 0 var(--s-6) var(--s-4) var(--s-8); }
  @media (max-width: 600px) { .r { flex-wrap: wrap; } }
</style>
