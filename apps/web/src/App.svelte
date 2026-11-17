<script lang="ts">
  // Routing: the first path segment is the kind (docs/addresses.md); the app that owns that kind fills the shell, the second segment is
  // the item. `/` goes to Threads. A kind no app handles shows the same not-found answer for missing and forbidden things.
  import { onMount } from "svelte";
  import { APPS, appFor, type AppDef } from "./apps/registry";
  import { navigate, route, segments, setQuery } from "./lib/route.svelte";
  import Shell from "./shell/Shell.svelte";
  import { shell } from "./shell/shell.svelte";
  import Unknown from "./shell/Unknown.svelte";

  const NONE: AppDef = { kind: "", name: "Calternal", icon: "apps", Sidebar: Unknown, Main: Unknown, panels: [{ id: "none", label: "Nothing", icon: "info", component: Unknown }] };
  const seg = $derived(segments());
  const app = $derived(appFor(seg[0]) ?? NONE);
  onMount(() => {
    if (route.path === "/") navigate("/threads", true);
    void Promise.all(APPS.map((a) => a.start?.()));
    const p = route.query.get("panel");
    if (p) shell.showPanel(p);
  });
  // Remember the open panel in the address so a link opens it (a transient parameter, never an identity).
  $effect(() => {
    if (shell.rightOpen && shell.panel && route.query.get("panel") !== shell.panel) setQuery("panel", shell.panel);
    if (!shell.rightOpen && route.query.has("panel")) setQuery("panel", null);
  });
</script>

{#if route.path !== "/"}<Shell {app} id={seg[1]} />{/if}
