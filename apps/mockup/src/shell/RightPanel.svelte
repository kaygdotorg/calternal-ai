<script lang="ts">
  import PanelFrame from "./PanelFrame.svelte";
  import { setContext } from "svelte";
  // The right panel (docs/ui-shell.md): flat on the background like the left sidebar, not a card. It belongs to the focused window: Details (what it is, its properties, who is on it, what happened) and Assistant (the AI chat that is in every app, here a
  // stand-in), with the two tabs at the bottom as decided. On a phone the same content is a sheet. Fixture only.
  import Av from "./Av.svelte";
  import { arr } from "../arrange.svelte";
  import { panels } from "../apps/registry";
  import { KIND_APP } from "../data/collections";
  import { ITEMS } from "../data/collections";
  import { APPS, SETTINGS } from "../data/apps";
  import type { Win } from "../windows.svelte";
  let { win, onclose, sheet = false }: { win: Win | undefined; onclose?: () => void; sheet?: boolean } = $props();
  setContext("panel", { get onclose() { return onclose; }, get sheet() { return sheet; }, get kind() { return item ? KINDNAME[item.kind] : coll ? "Collection" : win?.kind === "app" ? appName : undefined; } });
  const item = $derived(win ? (win.kind === "item" ? ITEMS[win.ref] : win.sel ? ITEMS[win.sel] : win.kind === "app" && win.ref === "chat" ? ITEMS["c-audit"] : undefined) : undefined);
  const KINDNAME: Record<string, string> = { chat: "Conversation", mail: "Email", note: "Note", task: "Task", file: "File", photo: "Photo", event: "Event", contact: "Contact", transaction: "Transaction", workout: "Workout", label: "Label", setting: "Setting" };
  const PROV: Record<string, string> = { claude: "Claude", codex: "Codex", gemini: "Gemini", local: "A local model" };
  const cap = (v: string) => v[0]!.toUpperCase() + v.slice(1);
  const sb = $derived(win?.kind === "collection" && win.ref.startsWith("sb:"));
  const coll = $derived(win?.kind === "collection" && !sb ? arr.coll(win.ref) : undefined);
  const appName = $derived(win?.kind === "app" ? ([...APPS, SETTINGS].find((a) => a.id === win.ref)?.name ?? arr.custom.find((c) => c.id === win.ref)?.name ?? "") : "");
  const title = $derived(item?.title ?? (sb ? "Sidebar" : coll?.name) ?? appName ?? "Nothing open");
  const rows = $derived.by((): [string, string][] => {
    if (item) {
      const k: Record<string, [string, string][]> = { mail: [["From", item.title], ["To", "Ada Lindqvist"], ["Received", item.when ?? ""]], task: [["Due", item.due ?? ""], ["Status", arr.statusOf(item.id) ?? "To do"], ["List", item.sub ?? ""]], transaction: [["Amount", item.amount ?? ""], ["Account", "Household"], ["Category", "Travel"]], event: [["When", item.sub ?? ""], ["Where", "Lisbon"], ["Calendar", "Personal"]], photo: [["Taken", item.when ?? "12 Nov"], ["Place", item.sub ?? ""], ["Camera", "Phone"]], workout: [["Distance", item.sub ?? ""], ["Time", item.when ?? ""], ["Heart rate", "142 bpm"]], file: [["Size", item.sub ?? ""], ["Type", "Document"], ["Folder", "Documents"]], contact: [["Role", item.sub ?? ""], ["Mobile", "+351 912 000 000"]], chat: [["With", item.title], ["Notifications", item.muted ? "Muted" : "On"], ["Agent", item.provider ? PROV[item.provider] : "None"]] };
      return [["Kind", KINDNAME[item.kind] ?? item.kind], ...(k[item.kind] ?? [["Modified", item.when ?? "Today"]]), ["Tags", "#travel, #lisbon"]];
    }
    if (coll) return [["Items", String(arr.ids(coll.id).length)], ["Opens as", cap(coll.big)], ["In a sidebar", cap(coll.small)], ["Shared with", "Household"], ["Stored as", `collections/${coll.id}.md`]];
    if (sb) return [["Kind", "Sidebar (a collection of collections)"], ["Sections", String(arr.sections(win!.ref.slice(3)).length)]];
    return win ? [["App", appName]] : [];
  });
  const people = $derived(item?.kind === "chat" || item?.kind === "mail" || coll ? ["Ada Lindqvist", "Ivo Novak", "Hana Sato"] : ["Ada Lindqvist"]);
</script>

{#if win && panels[win.kind === "app" ? win.ref : (KIND_APP[item?.kind ?? "label"] ?? "")]}
  {@const P = panels[win.kind === "app" ? win.ref : (KIND_APP[item?.kind ?? "label"] ?? "")]!}<P {win} {item} />
{:else}
<PanelFrame {title}>
  {#if rows.length}<section><h3>Properties</h3><dl>{#each rows as [k, v] (k)}<dt>{k}</dt><dd title={v}>{v}</dd>{/each}</dl></section>{/if}
  {#if people.length}<section><h3>Participants</h3><ul>{#each people as p, i (p)}<li><Av name={p} size="24px" /><span class="pn">{p}</span><span class="rl">{i === 0 ? "Owner" : "Editor"}</span></li>{/each}</ul></section>{/if}
  {#if item || coll}<section><h3>Activity</h3><ul class="act"><li>Ivo edited this <span>2h ago</span></li><li>Hana commented <span>Yesterday</span></li><li>You created this <span>Monday</span></li></ul></section>{/if}
</PanelFrame>
{/if}

<style>
  .rp { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); box-sizing: border-box; gap: var(--s-3); } header { flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--s-3); padding: var(--s-5) var(--s-3) 0; } h2 { margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); }
  .bd { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none; display: grid; align-content: start; gap: var(--gap-before-head); padding: var(--s-4) var(--s-3); } h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-3) var(--s-6); margin: 0; font-size: var(--text-ui); line-height: 1.25; } dt { color: var(--ink-3); } dd { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-3); font-size: var(--text-ui); } li { display: flex; align-items: center; gap: var(--s-4); min-height: 28px; } .pn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .rl, .act span { color: var(--ink-3); font-size: var(--text-label); margin-inline-start: auto; }
  .ai { display: grid; gap: var(--s-3); } .b { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); line-height: 1.45; } .b.me { justify-self: end; background: var(--accent-soft); }
  .in input { width: 100%; box-sizing: border-box; min-height: 34px; padding: 0 var(--s-5); border-radius: var(--r-pill); border: 0; outline: 0; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font: inherit; font-size: var(--text-ui); } footer { flex: none; padding-bottom: var(--s-3); } .sheet { padding-top: var(--s-3); max-height: 70vh; }
</style>
