<script lang="ts">
  // Routing: the first path segment is the kind (docs/addresses.md); the app that owns that kind fills the shell, the second segment is
  // the item. `/` goes to Threads. A kind no app handles shows the same not-found answer for missing and forbidden things.
  import { onMount, untrack } from "svelte";
  import { appFor, type AppDef } from "./apps/registry";
  import { navigate, route, segments, setQuery } from "./lib/route.svelte";
  import { Toasts, undoLatest } from "@calternal/ui";
  import Shell from "./shell/Shell.svelte";
  import LinkBanner from "./shell/LinkBanner.svelte";
  import { shell } from "./shell/shell.svelte";
  import { spaces } from "./shell/spaces.svelte";
  import Unknown from "./shell/Unknown.svelte";
  import { shortcuts } from "./lib/shortcuts.svelte";
  import ShortcutsSheet from "./shell/ShortcutsSheet.svelte";
  import { client, store } from "./apps/threads/store.svelte";
  import ThreadSearch from "./apps/threads/ThreadSearch.svelte";
  import { messageSearch } from "./apps/threads/search.svelte";
  import SnoozePicker from "./apps/threads/SnoozePicker.svelte";

  const NONE: AppDef = { kind: "", name: "Calternal", icon: "apps", Sidebar: Unknown, Main: Unknown, panels: [{ id: "none", label: "Nothing", icon: "info", component: Unknown }] };
  const seg = $derived(segments());
  const app = $derived(appFor(seg[0]) ?? NONE);
  // An app starts (loads its data and its code) the first time it is opened; Threads always.
  const started = new Set<string>();
  $effect(() => { for (const a of [appFor("threads"), app]) if (a && !started.has(a.kind)) { started.add(a.kind); void a.start?.(); } });
  // Opening a thread of another space (from the palette, a link) makes that space current, so the sidebar lists it. Addresses never name a space.
  $effect(() => { const t = seg[0] === "threads" && seg[1] ? store.get(seg[1]) : undefined; if (t && t.spaceId !== untrack(() => spaces.current.id)) spaces.switchTo(t.spaceId); });
  // The commands, registered once. Each also exists as a button, a menu entry or a sheet, so a shortcut is never the only way.
  const threadIds = () => [...store.threads].filter((t) => !t.parentId && t.spaceId === spaces.current.id).sort((x, y) => y.updatedAt - x.updatedAt).map((t) => t.id);
  const step = (by: 1 | -1): boolean => {
    const ids = threadIds();
    const cur = segments()[0] === "threads" ? segments()[1] : undefined;
    if (!ids.length) return false;
    navigate(`/threads/${ids[Math.max(0, Math.min(ids.length - 1, (cur ? ids.indexOf(cur) : -1) + by))]}`);
    return true;
  };
  shortcuts.register([
    { id: "help", label: "Show keyboard shortcuts", group: "Help", where: "Anywhere", chord: "Mod+Slash", run: () => { shell.helpOpen = !shell.helpOpen; } },
    { id: "help-q", label: "Show keyboard shortcuts (?)", group: "Help", where: "Outside a text field", chord: "Shift+Slash", run: () => { shell.helpOpen = !shell.helpOpen; } },
    { id: "search", label: "Search", group: "Navigation", where: "Anywhere", chord: "Mod+KeyK", run: () => { shell.searchOpen = true; } },
    { id: "sidebar", label: "Show or hide the sidebar", group: "Navigation", where: "Anywhere", chord: "Mod+Backslash", run: () => shell.toggleSidebar() },
    { id: "panel", label: "Show or hide the side panel", group: "Navigation", where: "Anywhere", chord: "Mod+Shift+Backslash", run: () => shell.toggleRight() },
    { id: "settings", label: "Appearance settings", group: "Navigation", where: "Anywhere", chord: "Mod+Comma", run: () => navigate("/settings/appearance") },
    { id: "app1", label: "Open Threads", group: "Navigation", where: "Anywhere", chord: "Alt+Digit1", run: () => navigate("/threads") },
    { id: "app2", label: "Open Notes", group: "Navigation", where: "Anywhere", chord: "Alt+Digit2", run: () => navigate("/notes") },
    { id: "undo", label: "Undo the last archive or delete", group: "Threads", where: "Outside a text field", chord: "Mod+KeyZ", run: () => undoLatest() || false },
    { id: "msgsearch", label: "Search messages in threads", group: "Threads", where: "Anywhere", chord: "Mod+Shift+KeyF", run: () => { messageSearch.open = true; } },
    { id: "next", label: "Next thread", group: "Threads", where: "Anywhere", chord: "Alt+ArrowDown", run: () => step(1) },
    { id: "prev", label: "Previous thread", group: "Threads", where: "Anywhere", chord: "Alt+ArrowUp", run: () => step(-1) },
    { id: "composer", label: "Write a message", group: "Threads", where: "Outside a text field", chord: "Slash", run: () => { const f = document.getElementById("composer-field"); if (!f) return false; f.focus(); } },
    { id: "stop", label: "Stop the running turn", group: "Threads", where: "A running thread", chord: "Mod+Period", run: () => { const id = segments()[0] === "threads" ? segments()[1] : undefined; if (!id || store.get(id)?.state !== "running") return false; void client.stop(id); } },
  ]);
  onMount(() => {
    shortcuts.start();
    void spaces.load().catch(() => {}); // the real server's spaces (dots, current, personal); a no-op on fixtures
    if (route.path === "/") navigate("/threads", true);

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
<LinkBanner />
<ThreadSearch />
<SnoozePicker />
<Toasts bottom={shell.narrow ? "calc(var(--bar-h) + env(safe-area-inset-bottom) + 12px)" : 24} />
<ShortcutsSheet open={shell.helpOpen} onclose={() => (shell.helpOpen = false)} />
