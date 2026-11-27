<script lang="ts">
  /**
   * Declaration-driven host entry in `apps/web`, mounted on the shared mockup shell
   * (`@calternal/canvas-shell`: `Canvas`, `Sidebar`, `AppStrip`, `Grip`, `Windows`,
   * `layout`, `watchNewWindow`, and `rows.css`).
   *
   * Zero custom shell markup or CSS; zero hardcoded plugin or kind names.
   */
  import { onDestroy, onMount } from "svelte";
  import { applyScheme, currentScheme, Grip, Toasts, type MenuItem } from "@calternal/ui";
  import {
    AppStrip,
    Canvas,
    createWindows,
    layout,
    Sidebar,
    watchNewWindow,
    WIDTHS,
    type StripItem,
    type Win,
  } from "@calternal/canvas-shell";
  import "@calternal/canvas-shell/rows.css";
  import Backdrop from "../shell/Backdrop.svelte";
  import {
    fetchDeclarations,
    hasPendingLogout,
    hostCall,
    LiveClient,
    sessionStatus,
    signOutHost,
    subscribeAuthChannel,
    type ConnectionState,
  } from "./client";
  import {
    humanize,
    itemMenuActions,
    operationFormFields,
    resolveComposerSlot,
    resolvePickerSlot,
    rowTitle,
    type DerivedAction,
  } from "./format";
  import HostCollection from "./HostCollection.svelte";
  import HostFormSheet, { type FormFieldSpec } from "./HostFormSheet.svelte";
  import HostRecords, { type ActiveStreamRun } from "./HostRecords.svelte";
  import HostSidebar from "./HostSidebar.svelte";
  import HostSignIn from "./HostSignIn.svelte";
  import type {
    CollectionDecl,
    KindDecl,
    LiveItemMetadata,
    PluginDecl,
    RecordData,
    RowData,
    SessionInfo,
    SidebarDecl,
    SpaceDeclarations,
  } from "./types";

  let checkingSession = $state(true);
  let needsSetup = $state(false);
  let pendingLogout = $state(hasPendingLogout());
  let session = $state<SessionInfo | null>(null);
  let declarations = $state<SpaceDeclarations | null>(null);
  let loadError = $state("");
  let liveState = $state<ConnectionState>("closed");
  let scheme = $state<"light" | "dark">(currentScheme());

  let width = $state(typeof innerWidth !== "undefined" ? innerWidth : 1280);
  const narrow = $derived(width < 720);
  let pushed = $state(false);
  const hidden = $derived(!narrow && !layout.leftOpen);

  let activePluginId = $state<string | null>(null);
  let selectedCollectionId = $state<string | null>(null);
  let customSidebars = $state<{ id: string; name: string }[]>([]);
  let stripOrder = $state<string[]>([]);
  let collectionRows = $state<Record<string, RowData[]>>({});
  let itemsById = $state<Record<string, RowData>>({});
  let recordsByItem = $state<Record<string, RecordData[]>>({});
  let activeRun = $state<ActiveStreamRun | null>(null);
  let streamTimer: ReturnType<typeof setInterval> | null = null;

  const wins = createWindows((itemId) => {
    const row = itemsById[itemId];
    return row ? row.kind.split(".")[0] : undefined;
  });

  interface SheetConfig {
    title: string;
    submitLabel: string;
    fields?: FormFieldSpec[];
    confirmText?: string;
    danger?: boolean;
    onsubmit: (values: Record<string, string>) => Promise<void>;
  }
  let sheet = $state<SheetConfig | null>(null);

  let live: LiveClient | null = null;
  let unsubQueries: (() => void)[] = [];
  const itemSubs = new Map<string, () => void>();
  let unsubAuthChannel: (() => void) | null = null;

  const plugins = $derived<PluginDecl[]>(declarations?.plugins ?? []);
  const appPlugins = $derived(plugins.filter((p) => Object.keys(p.sidebars).length > 0));
  const activePlugin = $derived<PluginDecl | null>(
    appPlugins.find((p) => p.id === activePluginId) ?? appPlugins[0] ?? null,
  );
  const activeSidebar = $derived<SidebarDecl | null>(
    activePlugin ? Object.values(activePlugin.sidebars)[0] ?? null : null,
  );

  const stripItems = $derived.by<StripItem[]>(() => {
    const base: StripItem[] = [
      ...appPlugins.map((p) => ({ id: p.id, name: p.name, icon: p.icon || "apps" })),
      ...customSidebars.map((c) => ({ id: c.id, name: c.name, icon: "sidebar" })),
    ];
    if (!stripOrder.length) return base;
    const byId = new Map(base.map((x) => [x.id, x]));
    const ordered: StripItem[] = [];
    for (const id of stripOrder) {
      const it = byId.get(id);
      if (it) { ordered.push(it); byId.delete(id); }
    }
    return [...ordered, ...byId.values()];
  });

  const sidebarTitle = $derived(
    activePluginId
      ? (stripItems.find((x) => x.id === activePluginId)?.name ?? "calternal")
      : "calternal",
  );

  function findCollection(qualified: string): CollectionDecl | null {
    const [pid, cid] = qualified.split(".");
    return plugins.find((p) => p.id === pid)?.collections[cid!] ?? null;
  }

  function findKind(qualified: string): KindDecl | null {
    const [pid, kid] = qualified.split(".");
    return plugins.find((p) => p.id === pid)?.kinds[kid!] ?? null;
  }

  function defaultCollectionFor(plugin: PluginDecl): CollectionDecl | null {
    const sb = Object.values(plugin.sidebars)[0];
    const entry =
      sb?.entries.find(
        (e) => e.type === "collection" && e.collection && plugin.collections[e.collection.split(".")[1]!]?.views[0]?.type === "list",
      ) ?? sb?.entries.find((e) => e.type === "collection");
    return entry?.collection ? findCollection(entry.collection) : Object.values(plugin.collections)[0] ?? null;
  }

  function ensureItemSubscribed(itemId: string) {
    if (!live || itemSubs.has(itemId)) return;
    const unsub = live.subscribeRecords(itemId, {
      onSnapshot: (recs, meta) => {
        recordsByItem = { ...recordsByItem, [itemId]: recs };
        applyLiveMeta(itemId, meta);
      },
      onDelta: (recs, meta) => {
        const cur = recordsByItem[itemId] ?? [];
        if (recs.length) {
          const seen = new Set(cur.map((r) => r.anchor));
          const fresh = recs.filter((r) => !seen.has(r.anchor));
          if (fresh.length) recordsByItem = { ...recordsByItem, [itemId]: [...cur, ...fresh] };
        }
        applyLiveMeta(itemId, meta);
      },
      onAppend: (rec, version) => {
        const cur = recordsByItem[itemId] ?? [];
        if (!cur.some((r) => r.anchor === rec.anchor)) {
          recordsByItem = { ...recordsByItem, [itemId]: [...cur, rec] };
        }
        const existing = itemsById[itemId];
        if (existing) itemsById = { ...itemsById, [itemId]: { ...existing, version } };
      },
      onItemChange: (meta) => applyLiveMeta(itemId, meta),
    });
    itemSubs.set(itemId, unsub);
  }

  function applyLiveMeta(itemId: string, meta: LiveItemMetadata) {
    const existing = itemsById[itemId];
    if (existing) {
      itemsById = {
        ...itemsById,
        [itemId]: {
          ...existing,
          version: meta.version,
          fields: meta.fields,
          computed: meta.computed,
          state: meta.state,
          when: meta.when,
        },
      };
    }
  }

  // Keep open item windows subscribed to live records automatically.
  $effect(() => {
    const openIds = new Set<string>();
    for (const w of wins.list) {
      if (w.kind === "item") openIds.add(w.ref);
      else if (w.kind === "app" && w.sel) openIds.add(w.sel);
    }
    for (const id of openIds) ensureItemSubscribed(id);
    for (const [id, unsub] of itemSubs) {
      if (!openIds.has(id)) { unsub(); itemSubs.delete(id); }
    }
  });

  function stopStreamFixture() {
    if (streamTimer) { clearInterval(streamTimer); streamTimer = null; }
    if (activeRun) {
      activeRun = { ...activeRun, status: "stopping" };
      setTimeout(() => (activeRun = null), 150);
    }
  }

  function startStreamFixture(label: string, fullText: string) {
    if (streamTimer) clearInterval(streamTimer);
    const words = fullText.split(" ");
    let idx = 0;
    activeRun = { id: `demo-${Date.now()}`, label, text: "", status: "streaming" };
    streamTimer = setInterval(() => {
      idx += 1;
      if (!activeRun || idx > words.length) {
        if (streamTimer) clearInterval(streamTimer);
        streamTimer = null;
        return;
      }
      activeRun = { ...activeRun, text: words.slice(0, idx).join(" ") };
    }, 60);
  }

  function cleanupLive() {
    if (streamTimer) { clearInterval(streamTimer); streamTimer = null; }
    activeRun = null;
    for (const unsub of itemSubs.values()) unsub();
    itemSubs.clear();
    for (const u of unsubQueries) u();
    unsubQueries = [];
    live?.close();
    live = null;
  }

  async function bootSession(s: SessionInfo) {
    cleanupLive();
    loadError = "";
    try {
      const decl = await fetchDeclarations(s);
      declarations = decl;
      const firstPlugin = decl.plugins.find((p) => Object.keys(p.sidebars).length > 0) ?? decl.plugins[0] ?? null;
      activePluginId = firstPlugin?.id ?? null;

      live = new LiveClient(s, (st) => (liveState = st));
      (window as unknown as Record<string, unknown>).__hostLive = {
        forceReconnect: () => live?.forceReconnect(),
        state: () => liveState,
        startStreamDemo: (label: string, text: string) => startStreamFixture(label, text),
      };

      for (const p of decl.plugins) {
        for (const col of Object.values(p.collections)) {
          const cid = col.qualified;
          const unsub = live.subscribeQuery({ collection: cid }, (rows) => {
            collectionRows = { ...collectionRows, [cid]: rows };
            const nextItems = { ...itemsById };
            for (const r of rows) nextItems[r.id] = r;
            itemsById = nextItems;
          });
          unsubQueries.push(unsub);
        }
      }

      if (firstPlugin) {
        const defCol = defaultCollectionFor(firstPlugin);
        if (defCol) wins.navigate("collection", defCol.qualified);
      }
    } catch (err) {
      loadError = err instanceof Error ? err.message : "Could not connect to host.";
    }
  }

  function openItem(row: RowData, collectionId?: string) {
    itemsById = { ...itemsById, [row.id]: row };
    selectedCollectionId = collectionId ?? null;
    if (wins.focused?.kind === "collection") {
      wins.navigate("item", row.id);
    } else {
      wins.openItemById(row.id);
    }
    pushed = true;
  }

  function openCollection(cid: string) {
    wins.navigate("collection", cid);
    pushed = true;
  }

  function selectStrip(id: string | null) {
    activePluginId = id;
    if (!id) {
      if (wins.focused?.kind === "app") wins.close(wins.focused.id);
      return;
    }
    const p = appPlugins.find((x) => x.id === id);
    if (p) {
      const defCol = defaultCollectionFor(p);
      if (defCol) wins.navigate("collection", defCol.qualified);
      else wins.navigate("app", p.id);
    }
  }

  function placeStrip(id: string, index: number) {
    const ids = stripItems.map((x) => x.id);
    const from = ids.indexOf(id);
    if (from < 0) return;
    ids.splice(from, 1);
    ids.splice(Math.max(0, Math.min(ids.length, index)), 0, id);
    stripOrder = ids;
  }

  function moveStrip(id: string, by: number) {
    const ids = stripItems.map((x) => x.id);
    placeStrip(id, ids.indexOf(id) + by);
  }

  function addCustomSidebar(name: string): string {
    const id = `sb-${Date.now().toString(36)}`;
    customSidebars = [...customSidebars, { id, name }];
    return id;
  }

  function openAddSheet(kind: KindDecl) {
    const addOp = kind.operations.add;
    if (!addOp || !session) return;
    const fieldNames = addOp.fields.length ? addOp.fields : ["title"];
    const fields: FormFieldSpec[] = fieldNames.map((name) => ({
      name,
      label: humanize(name),
      type: kind.fields[name]?.type ?? "text",
      required: kind.fields[name]?.required ?? true,
    }));
    sheet = {
      title: `New ${kind.name}`,
      submitLabel: `Create ${kind.name}`,
      fields,
      onsubmit: async (vals) => {
        if (!session) return;
        const res = await hostCall<{ changes: { id: string; version: number }[] }>(
          session,
          "s.write",
          { changes: [{ op: "add", kind: kind.qualified, fields: vals }] },
          `add-${crypto.randomUUID()}`,
        );
        const createdId = res.changes?.[0]?.id;
        if (createdId) {
          const created: RowData = {
            id: createdId,
            kind: kind.qualified,
            fields: vals,
            computed: {},
            version: res.changes[0]?.version ?? 0,
          };
          openItem(created);
        }
      },
    };
  }

  async function runAction(row: RowData, action: DerivedAction) {
    if (!session) return;
    const op = action.operation;
    if (op.form === "set" && op.target) {
      if (op.target.startsWith("state.")) {
        const key = op.target.slice("state.".length);
        await hostCall(session, "s.state", { op: "set", item: row.id, key, value: op.value });
      } else {
        await hostCall(session, "s.write", {
          changes: [{ op: "change", item: row.id, fields: { [op.target]: op.value } }],
        });
      }
      return;
    }
    if (op.form === "change" && op.target) {
      await hostCall(session, "s.write", {
        changes: [{ op: "change", item: row.id, fields: { [op.target]: op.value } }],
      });
      return;
    }
    if (op.form === "code") {
      const fields = operationFormFields(op, row);
      const qualifiedOp = `${action.kind.plugin}.${action.kind.id}.${op.name}`;
      if (!fields.length) {
        await hostCall(session, qualifiedOp, { item: row.id }, `op-${crypto.randomUUID()}`);
        return;
      }
      sheet = {
        title: action.label,
        submitLabel: action.label,
        fields,
        onsubmit: async (vals) => {
          if (!session) return;
          await hostCall(session, qualifiedOp, { item: row.id, ...vals }, `op-${crypto.randomUUID()}`);
        },
      };
    }
  }

  async function sendRecord(item: RowData, kind: KindDecl, body: string, quoteAnchor?: string, selectedFiller?: string | null) {
    const plugin = plugins.find((p) => p.id === kind.plugin);
    const comp = plugin ? resolveComposerSlot(plugin, kind) : null;
    const picker = plugin ? resolvePickerSlot(plugin, plugins, comp?.slot.name) : null;
    if (!session || !comp) return;
    const qualifiedOp = `${kind.plugin}.${kind.id}.${comp.operation.name}`;
    const input: Record<string, unknown> = { item: item.id, [comp.textInput]: body };
    if (quoteAnchor && comp.quoteInput) input[comp.quoteInput] = quoteAnchor;
    const slotSelection = selectedFiller && picker ? { slot: picker.slot.name, filler: selectedFiller } : undefined;
    await hostCall(session, qualifiedOp, input, `send-${crypto.randomUUID()}`, slotSelection);
  }

  function openEditRecord(itemId: string, rec: RecordData) {
    if (!session) return;
    sheet = {
      title: "Edit record",
      submitLabel: "Save",
      fields: [{ name: "body", label: "Text", type: "long text", required: true, value: rec.body }],
      onsubmit: async (vals) => {
        if (!session) return;
        await hostCall(session, "s.write", {
          changes: [{ op: "change record", item: itemId, anchor: rec.anchor, record: { body: vals.body } }],
        });
      },
    };
  }

  function openRemoveRecord(itemId: string, rec: RecordData) {
    if (!session) return;
    sheet = {
      title: "Remove record",
      submitLabel: "Remove",
      confirmText: "Remove this record? It will be redacted from the view.",
      danger: true,
      onsubmit: async () => {
        if (!session) return;
        await hostCall(session, "s.write", {
          changes: [{ op: "remove record", item: itemId, anchor: rec.anchor }],
        });
      },
    };
  }

  function windowTitle(w: Win): string {
    if (w.kind === "collection") return findCollection(w.ref)?.title ?? "";
    if (w.kind === "item") {
      const row = itemsById[w.ref];
      return row ? rowTitle(row, findKind(row.kind) ?? undefined) : "";
    }
    return plugins.find((p) => p.id === w.ref)?.name ?? "";
  }

  function windowExtraMenu(w: Win): MenuItem[] {
    if (w.kind !== "item") return [];
    const row = itemsById[w.ref];
    const kind = row ? findKind(row.kind) : null;
    if (!row || !kind) return [];
    return itemMenuActions(row, kind).map((a) => ({ id: `op:${a.id}`, label: a.label, danger: a.danger }));
  }

  function windowMenuPick(w: Win, id: string) {
    if (!id.startsWith("op:") || w.kind !== "item") return;
    const row = itemsById[w.ref];
    const kind = row ? findKind(row.kind) : null;
    if (!row || !kind) return;
    const act = itemMenuActions(row, kind).find((a) => `op:${a.id}` === id);
    if (act) void runAction(row, act);
  }

  function toggleScheme() {
    scheme = scheme === "dark" ? "light" : "dark";
    applyScheme(scheme);
  }

  function resetToSignedOut() {
    cleanupLive();
    wins.clear();
    session = null;
    declarations = null;
    collectionRows = {};
    itemsById = {};
    recordsByItem = {};
    needsSetup = false;
    pushed = false;
  }

  async function handleSignOut() {
    cleanupLive();
    const { offline } = await signOutHost();
    pendingLogout = offline;
    resetToSignedOut();
  }

  const accountItems = $derived<MenuItem[]>([
    { id: "scheme", label: scheme === "dark" ? "Light appearance" : "Dark appearance", icon: scheme === "dark" ? "sun" : "moon" },
    { id: "out", label: "Sign out", icon: "signout", danger: true },
  ]);
  const spaceItems = $derived<MenuItem[]>([
    { id: "personal", label: "Personal", sub: "Only you", icon: "folder", checked: true },
  ]);

  onMount(() => {
    const onResize = () => (width = innerWidth);
    addEventListener("resize", onResize);
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.ctrlKey && e.code === "KeyS") {
        e.preventDefault();
        layout.leftOpen = !layout.leftOpen;
      }
    };
    addEventListener("keydown", onKey);
    const stopIntent = watchNewWindow(
      wins,
      (id) => (activePluginId = id),
      () => !narrow && !!session,
      (id) => id in itemsById,
    );

    const qScheme = new URLSearchParams(location.search).get("scheme");
    if (qScheme === "dark" || qScheme === "light") {
      scheme = qScheme;
      applyScheme(qScheme);
    }

    unsubAuthChannel = subscribeAuthChannel(() => resetToSignedOut());

    void sessionStatus()
      .then((status) => {
        needsSetup = status.setup;
        pendingLogout = hasPendingLogout();
        if (status.session) {
          session = status.session;
          return bootSession(status.session);
        }
      })
      .catch((err) => {
        loadError = err instanceof Error ? err.message : "Could not reach host.";
      })
      .finally(() => {
        checkingSession = false;
      });

    return () => {
      removeEventListener("resize", onResize);
      removeEventListener("keydown", onKey);
      stopIntent();
      unsubAuthChannel?.();
    };
  });

  onDestroy(() => cleanupLive());
</script>

<Backdrop />
<Toasts bottom={24} />

{#if checkingSession}
  <main class="boot-wait" aria-busy="true" aria-label="Loading">
    <p class="mark">calternal</p>
  </main>
{:else if !session}
  <HostSignIn
    {needsSetup}
    {pendingLogout}
    onsignedin={(s) => {
      needsSetup = false;
      pendingLogout = false;
      session = s;
      void bootSession(s);
    }}
  />
{:else}
  <div
    class="shell"
    class:narrow
    class:closed={hidden}
    data-shell
    data-pushed={pushed}
    data-live={liveState}
    style:--sw={hidden ? undefined : `${layout.left}px`}
  >
    <aside class="side" inert={hidden}>
      <Sidebar
        title={sidebarTitle}
        home={sidebarTitle === "calternal"}
        onhome={() => { activePluginId = null; wins.clear(); }}
        {narrow}
        cursive={true}
        accountName={session.name}
        {accountItems}
        onAccountPick={(id) => { if (id === "scheme") toggleScheme(); else if (id === "out") void handleSignOut(); }}
        spaceName="Personal"
        {spaceItems}
        onsearch={() => {}}
      >
        {#snippet body()}
          {#if activeSidebar && activePlugin}
            <HostSidebar
              sidebar={activeSidebar}
              collections={activePlugin.collections}
              kinds={activePlugin.kinds}
              rowsByCollection={collectionRows}
              selectedItemId={wins.selected}
              {selectedCollectionId}
              {narrow}
              onopenItem={openItem}
              onopenCollection={openCollection}
              onadd={openAddSheet}
            />
          {/if}
        {/snippet}
      </Sidebar>
    </aside>

    {#if !narrow && !hidden}
      <Grip
        label="Resize the sidebar"
        min={WIDTHS.left.min}
        max={WIDTHS.left.max}
        now={layout.left}
        style="left: calc(var(--sw) - var(--s-7) / 2 - 2px)"
        onmove={(e) => layout.setWidth("left", e.clientX - 4)}
        onkey={(e) => {
          const d = e.shiftKey ? 64 : 16;
          let w = layout.left;
          if (e.key === "ArrowRight") w += d;
          else if (e.key === "ArrowLeft") w -= d;
          else if (e.key === "Home") w = WIDTHS.left.min;
          else if (e.key === "End") w = WIDTHS.left.max;
          else if (e.key === "Enter") { layout.setWidth("left", null); e.preventDefault(); return; }
          else return;
          e.preventDefault();
          layout.setWidth("left", w);
        }}
        onreset={() => layout.setWidth("left", null)}
      />
    {/if}

    <main class="mid" data-dock="show">
      {#if loadError}
        <p class="err" role="alert">{loadError}</p>
      {:else}
        <Canvas
          {wins}
          titleOf={windowTitle}
          extraMenu={windowExtraMenu}
          onMenuPick={windowMenuPick}
          {narrow}
          sidebarHidden={hidden}
          onsidebar={() => (layout.leftOpen = true)}
          onback={() => (pushed = false)}
          loneWidth="100%"
        >
          {#snippet renderBody(w)}
            {#if w.kind === "collection"}
              {@const col = findCollection(w.ref)}
              {@const kind = col ? findKind(col.from) : null}
              {#if col}
                <HostCollection
                  collection={col}
                  {kind}
                  rows={collectionRows[w.ref] ?? []}
                  selectedId={wins.selected}
                  onopen={openItem}
                  onadd={openAddSheet}
                  onaction={runAction}
                />
              {/if}
            {:else if w.kind === "item"}
              {@const row = itemsById[w.ref]}
              {@const kind = row ? findKind(row.kind) : null}
              {@const plugin = kind ? plugins.find((p) => p.id === kind.plugin) ?? null : null}
              {@const comp = plugin && kind ? resolveComposerSlot(plugin, kind) : null}
              {@const picker = plugin ? resolvePickerSlot(plugin, plugins, comp?.slot.name) : null}
              {#if row && kind}
                <HostRecords
                  {row}
                  {kind}
                  records={recordsByItem[row.id] ?? []}
                  viewerId={session?.account ?? ""}
                  composer={comp}
                  pickerSlot={picker}
                  {activeRun}
                  onsend={(body, quote, filler) => sendRecord(row, kind, body, quote, filler)}
                  onstoprun={stopStreamFixture}
                  oneditrecord={(rec) => openEditRecord(row.id, rec)}
                  onremoverecord={(rec) => openRemoveRecord(row.id, rec)}
                />
              {/if}
            {/if}
          {/snippet}
        </Canvas>
      {/if}
      {#if !narrow}
        <AppStrip
          items={stripItems}
          current={activePluginId}
          onselect={selectStrip}
          onplace={placeStrip}
          onmove={moveStrip}
          onadd={addCustomSidebar}
        />
      {/if}
    </main>

    {#if narrow && !pushed}
      <AppStrip
        items={stripItems}
        current={activePluginId}
        onselect={selectStrip}
        onplace={placeStrip}
        onmove={moveStrip}
        onadd={addCustomSidebar}
        narrow
      />
    {/if}
  </div>

  {#if sheet}
    <HostFormSheet
      title={sheet.title}
      submitLabel={sheet.submitLabel}
      fields={sheet.fields}
      confirmText={sheet.confirmText}
      danger={sheet.danger}
      onsubmit={sheet.onsubmit}
      onclose={() => (sheet = null)}
    />
  {/if}
{/if}

<style>
  .boot-wait { min-height: 100dvh; display: grid; place-content: center; position: relative; z-index: 1; }
  .mark { font-family: var(--wm-font, var(--font-ui)); font-size: var(--text-title); font-weight: var(--strong-weight); color: var(--ink); margin: 0; }
  .err { margin: auto; padding: var(--s-6); color: var(--danger); }

  /* Exact shell grid from apps/mockup/src/App.svelte */
  .shell { position: fixed; inset: 0; z-index: 1; display: grid; grid-template-columns: var(--sw, 19rem) minmax(0, 1fr); gap: var(--s-3); padding: var(--s-3) var(--s-3) var(--s-3) 0; box-sizing: border-box; transition: grid-template-columns var(--dur-slow) var(--ease-out); }
  .shell.closed { grid-template-columns: 0 minmax(0, 1fr); padding-left: var(--s-3); }
  .shell.closed .side { opacity: 0; translate: -2rem 0; }
  .side { --ink-2: var(--l-ink2, currentcolor); --ink-3: var(--l-ink3, currentcolor); min-width: 0; min-height: 0; overflow: hidden; transition: opacity var(--dur-base) var(--ease-out), translate var(--dur-slow) var(--ease-out); }
  .mid { --ink-2: var(--c-ink2, currentcolor); --ink-3: var(--c-ink3, currentcolor); position: relative; min-width: 0; min-height: 0; display: block; --dock-space: calc(var(--tb-h) + var(--s-5) + var(--s-5) - var(--s-3)); box-sizing: border-box; }
  .shell:not(.narrow) .mid { margin-block: calc(var(--s-3) * -1); padding-bottom: var(--dock-space); transition: padding-bottom var(--dur-morph) var(--ease-morph); }
  .narrow { display: block; padding: 0; }
  .narrow .side, .narrow .mid { position: absolute; inset: 0; transition: translate var(--dur-slow) var(--ease-out); }
  .narrow .mid { padding: var(--s-3); box-sizing: border-box; translate: 100% 0; }
  .narrow[data-pushed="true"] .mid { translate: 0 0; }
  .narrow[data-pushed="true"] .side { translate: -20% 0; opacity: 0; visibility: hidden; transition: translate var(--dur-slow) var(--ease-out), opacity var(--dur-slow) var(--ease-out), visibility 0s linear var(--dur-slow); }
</style>
