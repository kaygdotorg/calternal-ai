<script lang="ts">
  import { Canvas as SharedCanvas, type Win } from "@calternal/canvas-shell";
  import WindowBody from "./WindowBody.svelte";
  import RightPanel from "../shell/RightPanel.svelte";
  import { wins } from "../windows.svelte";
  import { ITEMS, type Item } from "../data/collections";
  import { APPS, SETTINGS } from "../data/apps";
  import { arr } from "../arrange.svelte";
  import { titles } from "../apps/registry";
  import { tweaks } from "../tweaks.svelte";

  let { narrow, onopen, oncollection, onback, lights = false, sidebarHidden = false, onsidebar }: {
    sidebarHidden?: boolean; onsidebar?: () => void; narrow: boolean; onopen: (i: Item) => void; oncollection: (cid: string) => void; onback: () => void; lights?: boolean;
  } = $props();

  const LONE: Record<string, string> = { "640": "640px", "800": "800px", "960": "960px", fill: "100%" };
  const it = (i: Item | undefined) => (i ? (i.kind === "mail" ? (i.sub ?? i.title) : i.title) : undefined);
  const titleOf = (w: Win) =>
    w.kind === "collection"
      ? (w.ref.startsWith("sb:")
          ? (arr.custom.find((c) => c.id === w.ref.slice(3))?.name ?? `${w.ref.slice(3)[0]!.toUpperCase()}${w.ref.slice(4)}`) + " (sidebar)"
          : (arr.coll(w.ref)?.name ?? ""))
      : w.kind === "item"
        ? (it(ITEMS[w.ref]) ?? "")
        : ((titles[w.ref]?.(w) || undefined) ?? (w.sel ? it(ITEMS[w.sel]) : undefined) ?? [...APPS, SETTINGS].find((a) => a.id === w.ref)?.name ?? arr.custom.find((c) => c.id === w.ref)?.name ?? "");
</script>

<SharedCanvas
  {wins}
  {titleOf}
  {narrow}
  {lights}
  {sidebarHidden}
  {onsidebar}
  {onback}
  loneWidth={LONE[tweaks.t.loneW] ?? "800px"}
  showRightToggle={true}
>
  {#snippet renderBody(w)}
    <WindowBody win={w} {onopen} {oncollection} />
  {/snippet}
  {#snippet rightSheet(focused, close)}
    <RightPanel win={focused} sheet onclose={close} />
  {/snippet}
</SharedCanvas>
