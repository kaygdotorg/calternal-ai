<script lang="ts">
  import { TextMorph } from "torph/svelte";
  import { Sidebar as SharedSidebar, tween } from "@calternal/canvas-shell";
  import type { MenuItem } from "@calternal/ui";
  import SidebarBody from "../sidebars/SidebarBody.svelte";
  import type { Item } from "../data/collections";
  import { ME } from "../data/apps";
  import { share } from "../share.svelte";
  import SidebarMenu from "./SidebarMenu.svelte";
  import NewSpace from "./NewSpace.svelte";
  import PluginSheet from "./PluginSheet.svelte";
  import { tweaks, panel, forced } from "../tweaks.svelte";

  let { title, home, onhome, app, onopen, onapp, onaction, oncollection, onsearch, onsettings, selected, narrow, mac }: {
    title: string; home: boolean; onhome: () => void; app: string | null; onopen: (i: Item) => void; onapp?: () => void; onaction?: () => void; oncollection: (cid: string) => void; onsearch: () => void; onsettings: () => void; selected: string | null; narrow: boolean; mac: boolean;
  } = $props();

  let newSpace = $state(false), plugins = $state(forced.sheet === "plugins");
  const spaceName = $derived(share.space.name);
  const accountItems = $derived<MenuItem[]>([
    { id: "settings", label: "Settings", icon: "settings" },
    { id: "tweaks", label: "Mockup tweaks", icon: "wrench" },
    { id: "out", label: "Sign out", icon: "signout", danger: true },
  ]);
  const spaceItems = $derived<MenuItem[]>([
    ...share.spaces.map((s) => ({ id: `space:${s.id}`, label: s.name, sub: s.members ? `${s.members} members` : "Only you", icon: "folder" as const, checked: s.id === share.current })),
    { id: "new", label: "New space…", icon: "plus" as const },
    { id: "plugins", label: "Add a plugin…", icon: "apps" as const },
  ]);
  const onAccountPick = (id: string) => { if (id === "settings") onsettings(); else if (id === "tweaks") panel.open = true; };
  const onSpacePick = (id: string) => { if (id.startsWith("space:")) share.current = id.slice(6); else if (id === "new") newSpace = true; else if (id === "plugins") plugins = true; };
  const cursive = $derived(home || tweaks.t.appName === "cursive");
  const t = $derived(tween("--dur-morph", 350));
</script>

<SharedSidebar
  {title}
  {home}
  {onhome}
  {narrow}
  {mac}
  {cursive}
  accountName={ME.name}
  {accountItems}
  {onAccountPick}
  {spaceName}
  {spaceItems}
  {onSpacePick}
  spaceSearch={tweaks.t.profile === "dense"}
  {onsearch}
>
  {#snippet titleSlot()}
    <TextMorph text={title} duration={t.duration} respectReducedMotion={true} ease="cubic-bezier(0.23, 1, 0.32, 1)" as="span" />
  {/snippet}
  {#snippet headerActions()}
    {#if app}<SidebarMenu sid={app} name={title} onfile={() => oncollection(`sb:${app}`)} />{/if}
  {/snippet}
  {#snippet body()}
    <SidebarBody sid={app} {selected} {onopen} {oncollection} {narrow} appName={title} {onapp} {onaction} />
  {/snippet}
  {#snippet extra()}
    <NewSpace open={newSpace} onclose={() => (newSpace = false)} />
    <PluginSheet open={plugins} onclose={() => (plugins = false)} start={forced.plugin} />
  {/snippet}
</SharedSidebar>
