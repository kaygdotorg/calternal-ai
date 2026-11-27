<script lang="ts">
  // The sidebar's ⋯ menu (top line) and the sheets behind it: "Make this the space's sidebar" (Owner and Managers), "Reset to the space's sidebar" (with Undo), "View file", and a look at the two notices other people get. Fixture only: it changes the mockup's own arrangement and nothing else.
  import { IconButton, Menu, Panel, Sheet, toast, type MenuItem } from "@calternal/ui";
  import { arr } from "../arrange.svelte";
  import { share } from "../share.svelte";
  import { tweaks } from "../tweaks.svelte";
  let { sid, name, onfile }: { sid: string; name: string; onfile: () => void } = $props();
  let open = $state(false), trig = $state<HTMLElement | null>(null), sheet = $state<"share" | "notices" | null>(null);
  $effect(() => { share.ensure(sid); });
  const custom = $derived(share.customized(sid)), space = $derived(share.space), canShare = $derived(tweaks.t.role === "Owner" || tweaks.t.role === "Manager");
  const own = $derived(space.members === 0);
  const items = $derived<MenuItem[]>([
    ...(custom ? [{ id: "none", label: "Customized", sub: "Your own copy: nobody else sees it", icon: "edit" as const, disabled: true }] : []),
    { id: "share", label: "Make this the space's sidebar", sub: own ? "Personal has no members" : canShare ? undefined : "Owners and Managers only", icon: "share" as const, disabled: own || !canShare },
    { id: "reset", label: "Reset to the space's sidebar", icon: "restore" as const, disabled: !custom },
    { id: "file", label: "View file", icon: "file" as const },
    { id: "notices", label: "Show the notices others get", icon: "bell" as const },
  ]);
  function pick(id: string) {
    open = false;
    if (id === "share") sheet = "share"; else if (id === "notices") sheet = "notices"; else if (id === "file") onfile();
    else if (id === "reset") { const prev = share.reset(sid); toast("Back to the space's sidebar", { label: "Undo", run: () => share.restore(sid, prev) }); }
  }
  const rows = $derived(arr.sections(sid).filter((s) => arr.coll(s.cid) && !s.hidden));
  function confirm() { share.makeShared(sid); sheet = null; toast(`${name} is now the sidebar for ${space.name}`); }
  const KIM = "Kim";
</script>

<Panel radius="pill" class="roundP tbp"><span class="mb"><IconButton icon="more" label="Sidebar menu" bind:el={trig} onclick={() => (open = true)} />{#if custom}<i class="cd" title="Customized" aria-hidden="true"></i>{/if}</span></Panel>
<Menu {open} {items} label={`${name} sidebar`} onpick={pick} onclose={() => (open = false)} trigger={trig} placement="bottom" />

<Sheet open={sheet === "share"} label="Make this the space's sidebar" onclose={() => (sheet = null)} side="center">
  <div class="sh" data-share-sheet>
    <h2>Make this the space's sidebar</h2>
    <p class="intro">This is how {name} will look for everyone in {space.name}:</p>
    <ul class="pv" aria-label="Preview">{#each rows as r (r.cid)}<li><i style:--hue={arr.coll(r.cid)!.hue}></i>{r.name ?? arr.coll(r.cid)!.name}<span>{arr.items(r.cid).length}</span></li>{/each}{#if !rows.length}<li class="q">No sections yet.</li>{/if}</ul>
    <p class="note">Replaces the sidebar for {space.name} ({space.members} members). People who arranged their own keep theirs.</p>
    <div class="bt"><button type="button" onclick={() => (sheet = null)}>Cancel</button><button type="button" class="pri" onclick={confirm}>Make it the space's sidebar</button></div>
  </div>
</Sheet>

<Sheet open={sheet === "notices"} label="Notices others get" onclose={() => (sheet = null)} side="center">
  <div class="sh" data-notices>
    <h2>What other people see</h2>
    <p class="intro">When the space's sidebar changes, each member gets one quiet notice, once.</p>
    <div class="nt"><p><b>{KIM}</b> rearranged the sidebar</p><div><button type="button" onclick={() => (sheet = null)}>Keep the old one for me</button></div><small>Shown to people who did not arrange their own. It changed under them; they can keep what they had.</small></div>
    <div class="nt"><p><b>{KIM}</b> shared a new sidebar</p><div><button type="button" class="pri" onclick={() => (sheet = null)}>Use it</button><button type="button" onclick={() => (sheet = null)}>Keep mine</button></div><small>Shown to people who arranged their own: nothing changes until they choose.</small></div>
    <div class="bt"><button type="button" onclick={() => (sheet = null)}>Close</button></div>
  </div>
</Sheet>

<style>
  .mb { position: relative; display: grid; place-items: center; } .cd { position: absolute; top: 6px; right: 6px; width: 8px; height: 8px; border-radius: 50%; background: var(--accent-base); pointer-events: none; }
  .sh { display: grid; gap: var(--s-5); padding: var(--s-7); width: min(30rem, 92vw); box-sizing: border-box; } h2 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); } p { margin: 0; color: var(--ink-2); font-size: var(--text-ui); line-height: 1.35; } .note { color: var(--ink-2); }
  .pv { list-style: none; margin: 0; padding: var(--s-3); display: grid; gap: var(--s-1); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.06); max-height: 14rem; overflow: auto; }
  .pv li { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-4); font-size: var(--text-ui); color: var(--ink); } .pv li i { width: 12px; height: 12px; border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); flex: none; } .pv li span { margin-inline-start: auto; color: var(--ink-3); font-size: var(--text-label); } .pv .q { color: var(--ink-3); }
  .bt { display: flex; justify-content: flex-end; gap: var(--s-4); } button:not(.ib) { min-height: var(--row-h); padding: 0 var(--s-6); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font-size: var(--text-ui); } button.pri { background: var(--badge-bg); color: #fff; }
  .nt { display: grid; gap: var(--s-3); padding: var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.06); } .nt p { color: var(--ink); } .nt div { display: flex; flex-wrap: wrap; gap: var(--s-3); } .nt small { color: var(--ink-3); font-size: var(--text-label); line-height: 1.35; }
</style>
