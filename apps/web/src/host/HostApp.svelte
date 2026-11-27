<script lang="ts">
  /**
   * Declaration-driven browser shell for the new host (slices 4 & 5).
   *
   * Look and feel follows `origin/lane/threads-ui` (`apps/mockup`):
   * - Bare wallpaper canvas (`Backdrop`), no top bands.
   * - Floating glass left sidebar drawn from `sidebars/*.md`.
   * - Rounded window cards (`--r-pane` / `--r-card`) with floating top-left title pill
   *   and floating top-right operations pill.
   * - Floating bottom-centre app strip.
   * - Nothing in this file or its children names any specific plugin or kind.
   */
  import { onDestroy, onMount } from "svelte";
  import { applyScheme, currentScheme, Icon, Menu, type MenuItem } from "@calternal/ui";
  import Backdrop from "../shell/Backdrop.svelte";
  import {
    fetchDeclarations,
    hostCall,
    LiveClient,
    loadSession,
    signOutHost,
    type ConnectionState,
  } from "./client";
  import { humanize, itemMenuActions, rowTitle, type DerivedAction } from "./format";
  import HostCollection from "./HostCollection.svelte";
  import HostFormSheet, { type FormFieldSpec } from "./HostFormSheet.svelte";
  import HostRecords from "./HostRecords.svelte";
  import HostSidebar from "./HostSidebar.svelte";
  import HostSignIn from "./HostSignIn.svelte";
  import type {
    CollectionDecl,
    FillDecl,
    KindDecl,
    PluginDecl,
    RecordData,
    RowData,
    SessionInfo,
    SidebarDecl,
    SpaceDeclarations,
  } from "./types";

  let session = $state<SessionInfo | null>(loadSession());
  let declarations = $state<SpaceDeclarations | null>(null);
  let loadError = $state("");
  let liveState = $state<ConnectionState>("closed");
  let scheme = $state<"light" | "dark">(currentScheme());

  let activePluginId = $state<string | null>(null);
  let activeCollectionId = $state<string | null>(null);
  let openItem = $state<RowData | null>(null);
  let openRecords = $state<RecordData[]>([]);
  let collectionRows = $state<Record<string, RowData[]>>({});
  let sidebarOpen = $state(true);
  let narrow = $state(typeof matchMedia !== "undefined" && matchMedia("(max-width: 759px)").matches);

  let headerMenuEl = $state<HTMLElement | null>(null);
  let headerMenuOpen = $state(false);

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
  let unsubRecords: (() => void) | null = null;

  const plugins = $derived<PluginDecl[]>(declarations?.plugins ?? []);
  const activePlugin = $derived<PluginDecl | null>(
    plugins.find((p) => p.id === activePluginId) ?? plugins[0] ?? null,
  );
  const activeSidebar = $derived<SidebarDecl | null>(
    activePlugin ? Object.values(activePlugin.sidebars)[0] ?? null : null,
  );

  function findCollection(qualified: string | null): CollectionDecl | null {
    if (!qualified) return null;
    const [pid, cid] = qualified.split(".");
    const p = plugins.find((x) => x.id === pid);
    return p?.collections[cid!] ?? null;
  }

  function findKind(qualified: string | null): KindDecl | null {
    if (!qualified) return null;
    const [pid, kid] = qualified.split(".");
    const p = plugins.find((x) => x.id === pid);
    return p?.kinds[kid!] ?? null;
  }

  const activeCollection = $derived<CollectionDecl | null>(findCollection(activeCollectionId));
  const activeCollectionKind = $derived<KindDecl | null>(
    activeCollection ? findKind(activeCollection.from) : null,
  );
  const openItemKind = $derived<KindDecl | null>(openItem ? findKind(openItem.kind) : null);

  const composerFill = $derived.by<FillDecl | null>(() => {
    if (!openItemKind || !activePlugin) return null;
    for (const fill of Object.values(activePlugin.fills)) {
      if (fill.kind === "operation") return fill;
    }
    return null;
  });

  const collectionCounts = $derived.by<Record<string, number>>(() => {
    const out: Record<string, number> = {};
    for (const [k, rows] of Object.entries(collectionRows)) {
      out[k] = rows.length;
    }
    return out;
  });

  const activeRows = $derived<RowData[]>(
    activeCollectionId ? collectionRows[activeCollectionId] ?? [] : [],
  );

  const itemActions = $derived<DerivedAction[]>(
    openItem && openItemKind ? itemMenuActions(openItem, openItemKind) : [],
  );
  const itemMenuItems = $derived<MenuItem[]>(
    itemActions.map((a) => ({ id: a.id, label: a.label, danger: a.danger })),
  );

  function cleanupLive() {
    unsubRecords?.();
    unsubRecords = null;
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
      const firstPlugin = decl.plugins[0] ?? null;
      activePluginId = firstPlugin?.id ?? null;
      const firstSidebar = firstPlugin ? Object.values(firstPlugin.sidebars)[0] ?? null : null;
      const firstCol = firstSidebar?.entries.find((e) => e.type === "collection")?.collection ?? null;
      activeCollectionId = firstCol;

      live = new LiveClient(s, (st) => (liveState = st));
      // Expose a minimal debug hook for Playwright reconnect-from-anchor tests.
      (window as unknown as Record<string, unknown>).__hostLive = {
        forceReconnect: () => live?.forceReconnect(),
        state: () => liveState,
      };

      // Subscribe to every collection referenced in the active plugin's sidebars.
      if (firstPlugin) {
        for (const col of Object.values(firstPlugin.collections)) {
          const cid = col.qualified;
          const unsub = live.subscribeQuery({ collection: cid }, (rows) => {
            collectionRows = { ...collectionRows, [cid]: rows };
            if (openItem) {
              const updated = rows.find((r) => r.id === openItem!.id);
              if (updated) openItem = updated;
            }
          });
          unsubQueries.push(unsub);
        }
      }
    } catch (err) {
      loadError = err instanceof Error ? err.message : "Could not connect to host.";
    }
  }

  function subscribeOpenItem(item: RowData | null) {
    unsubRecords?.();
    unsubRecords = null;
    openRecords = [];
    if (!item || !live) return;
    unsubRecords = live.subscribeRecords(item.id, {
      onSnapshot: (recs, meta) => {
        openRecords = recs;
        if (openItem && openItem.id === item.id) {
          openItem = { ...openItem, version: meta.version, fields: meta.fields, computed: meta.computed };
        }
      },
      onDelta: (recs, meta) => {
        if (recs.length) {
          const seen = new Set(openRecords.map((r) => r.anchor));
          const fresh = recs.filter((r) => !seen.has(r.anchor));
          if (fresh.length) openRecords = [...openRecords, ...fresh];
        }
        if (openItem && openItem.id === item.id) {
          openItem = { ...openItem, version: meta.version, fields: meta.fields, computed: meta.computed };
        }
      },
      onAppend: (rec, version) => {
        if (!openRecords.some((r) => r.anchor === rec.anchor)) {
          openRecords = [...openRecords, rec];
        }
        if (openItem && openItem.id === item.id) {
          openItem = { ...openItem, version };
        }
      },
      onItemChange: (meta) => {
        if (openItem && openItem.id === item.id) {
          openItem = { ...openItem, version: meta.version, fields: meta.fields, computed: meta.computed };
        }
      },
    });
  }

  function selectCollection(cid: string) {
    activeCollectionId = cid;
    if (narrow) {
      sidebarOpen = false;
      openItem = null;
      subscribeOpenItem(null);
    }
  }

  function selectItem(row: RowData) {
    openItem = row;
    subscribeOpenItem(row);
    if (narrow) sidebarOpen = false;
  }

  function closeItem() {
    openItem = null;
    subscribeOpenItem(null);
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
          selectItem(created);
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
      // Ask for the kind's primary text field (e.g. `title` for a rename operation).
      const textField =
        Object.values(action.kind.fields).find((f) => f.type === "text")?.name ?? "title";
      const currentVal = typeof row.fields[textField] === "string" ? (row.fields[textField] as string) : "";
      sheet = {
        title: action.label,
        submitLabel: action.label,
        fields: [
          {
            name: textField,
            label: humanize(textField),
            type: "text",
            required: true,
            value: currentVal,
          },
        ],
        onsubmit: async (vals) => {
          if (!session) return;
          const qualifiedOp = `${action.kind.plugin}.${action.kind.id}.${op.name}`;
          const singular = action.kind.id.replace(/s$/, "");
          await hostCall(
            session,
            qualifiedOp,
            { item: row.id, [singular]: row.id, ...vals },
            `op-${crypto.randomUUID()}`,
          );
        },
      };
    }
  }

  async function sendRecord(body: string, quoteAnchor?: string) {
    if (!session || !openItem || !openItemKind || !composerFill) return;
    const qualifiedOp = `${openItemKind.plugin}.${openItemKind.id}.${composerFill.target}`;
    const singular = openItemKind.id.replace(/s$/, "");
    const input: Record<string, unknown> = { item: openItem.id, [singular]: openItem.id, text: body };
    if (quoteAnchor) input.quote = quoteAnchor;
    await hostCall(session, qualifiedOp, input, `send-${crypto.randomUUID()}`);
  }

  function openEditRecord(rec: RecordData) {
    if (!session || !openItem) return;
    const itemId = openItem.id;
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

  function openRemoveRecord(rec: RecordData) {
    if (!session || !openItem) return;
    const itemId = openItem.id;
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

  function toggleScheme() {
    scheme = scheme === "dark" ? "light" : "dark";
    applyScheme(scheme);
  }

  async function handleSignOut() {
    const s = session;
    cleanupLive();
    session = null;
    declarations = null;
    openItem = null;
    openRecords = [];
    collectionRows = {};
    if (s) await signOutHost(s);
  }

  onMount(() => {
    const mq = matchMedia("(max-width: 759px)");
    const syncNarrow = () => {
      narrow = mq.matches;
      if (narrow) sidebarOpen = false;
    };
    mq.addEventListener("change", syncNarrow);
    if (narrow) sidebarOpen = false;

    const qScheme = new URLSearchParams(location.search).get("scheme");
    if (qScheme === "dark" || qScheme === "light") {
      scheme = qScheme;
      applyScheme(qScheme);
    }

    if (session) void bootSession(session);
    return () => mq.removeEventListener("change", syncNarrow);
  });

  onDestroy(() => cleanupLive());
</script>

<Backdrop />

{#if !session}
  <HostSignIn
    onsignedin={(s) => {
      session = s;
      void bootSession(s);
    }}
  />
{:else}
  <div class="host-shell" class:narrow data-live={liveState}>
    {#if sidebarOpen && activeSidebar}
      <aside class="side-slot" aria-label="Navigation">
        <HostSidebar
          sidebar={activeSidebar}
          activeCollection={activeCollectionId}
          counts={collectionCounts}
          addKind={activeCollectionKind}
          onselect={selectCollection}
          onadd={openAddSheet}
          onsignout={handleSignOut}
          accountName={session.name}
        />
      </aside>
    {/if}

    <main class="canvas" aria-label="Workspace">
      {#if loadError}
        <div class="card err-card" role="alert">
          <p>{loadError}</p>
        </div>
      {:else}
        <div class="windows" class:split={!narrow && !!openItem && !!activeCollection}>
          {#if activeCollection && (!narrow || !openItem)}
            <section class="card win" aria-label={activeCollection.title}>
              <div class="float-bar">
                <span class="title-pill">
                  {#if narrow}
                    <button
                      type="button"
                      class="pill-btn"
                      aria-label="Toggle sidebar"
                      onclick={() => (sidebarOpen = !sidebarOpen)}
                    >
                      <Icon name="sidebar" inline />
                    </button>
                  {/if}
                  <span class="pill-title">{activeCollection.title}</span>
                </span>
                {#if activeCollectionKind?.operations.add}
                  <div class="act-pill">
                    <button
                      type="button"
                      class="pill-btn"
                      aria-label={`New ${activeCollectionKind.name}`}
                      onclick={() => openAddSheet(activeCollectionKind!)}
                    >
                      <Icon name="plus" inline />
                    </button>
                  </div>
                {/if}
              </div>
              <HostCollection
                collection={activeCollection}
                kind={activeCollectionKind}
                rows={activeRows}
                selectedId={openItem?.id ?? null}
                onopen={selectItem}
                onadd={openAddSheet}
                onaction={runAction}
              />
            </section>
          {/if}

          {#if openItem && openItemKind}
            <section class="card win detail" aria-label={rowTitle(openItem, openItemKind)}>
              <div class="float-bar">
                <span class="title-pill">
                  <button
                    type="button"
                    class="pill-btn"
                    aria-label="Back"
                    onclick={closeItem}
                  >
                    <Icon name="back" inline />
                  </button>
                  <span class="pill-title" data-open-title>{rowTitle(openItem, openItemKind)}</span>
                </span>
                <div class="act-pill">
                  {#if itemMenuItems.length}
                    <button
                      type="button"
                      class="pill-btn"
                      bind:this={headerMenuEl}
                      aria-label="Item actions"
                      onclick={() => (headerMenuOpen = !headerMenuOpen)}
                    >
                      <Icon name="more" inline />
                    </button>
                  {/if}
                  {#if !narrow}
                    <button
                      type="button"
                      class="pill-btn"
                      aria-label="Close window"
                      onclick={closeItem}
                    >
                      <Icon name="close" inline />
                    </button>
                  {/if}
                </div>
              </div>
              <HostRecords
                row={openItem}
                kind={openItemKind}
                records={openRecords}
                viewerId={session.account}
                {composerFill}
                onsend={sendRecord}
                oneditrecord={openEditRecord}
                onremoverecord={openRemoveRecord}
              />
            </section>
          {/if}
        </div>
      {/if}

      <!-- Bottom-centre floating glass strip (matches mockup AppStrip) -->
      <nav class="strip" aria-label="Workspace controls">
        <button
          type="button"
          class="strip-btn"
          aria-label={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
          aria-pressed={sidebarOpen}
          onclick={() => (sidebarOpen = !sidebarOpen)}
        >
          <Icon name="sidebar" inline />
        </button>
        {#each plugins as p (p.id)}
          <button
            type="button"
            class="strip-btn app-btn"
            class:cur={activePlugin?.id === p.id}
            aria-label={p.name}
            aria-current={activePlugin?.id === p.id ? "page" : undefined}
            onclick={() => (activePluginId = p.id)}
          >
            <Icon name={(p.icon || "apps") as never} inline />
            <span class="strip-lbl">{p.name}</span>
          </button>
        {/each}
        <button
          type="button"
          class="strip-btn"
          aria-label={scheme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
          onclick={toggleScheme}
        >
          <Icon name={scheme === "dark" ? "sun" : "moon"} inline />
        </button>
      </nav>
    </main>
  </div>

  <Menu
    open={headerMenuOpen}
    items={itemMenuItems}
    label="Item actions"
    onpick={(id) => {
      headerMenuOpen = false;
      const act = itemActions.find((a) => a.id === id);
      if (act && openItem) void runAction(openItem, act);
    }}
    onclose={() => (headerMenuOpen = false)}
    trigger={headerMenuEl}
  />

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
  .host-shell {
    position: fixed;
    inset: 0;
    z-index: 1;
    display: flex;
    gap: var(--inset, 8px);
    padding: var(--inset, 8px);
    box-sizing: border-box;
    color: var(--ink);
  }
  .side-slot {
    width: var(--side-w, 240px);
    flex: none;
    min-height: 0;
    animation: slideIn var(--dur-base) var(--ease-out);
  }
  @keyframes slideIn {
    from { opacity: 0; translate: -10px 0; }
  }
  .canvas {
    flex: 1;
    min-width: 0;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .windows {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--inset, 8px);
    padding-bottom: calc(var(--tier-cap, 48px) + var(--s-4));
  }
  .windows.split {
    grid-template-columns: minmax(260px, 320px) minmax(0, 1fr);
  }
  .card {
    position: relative;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-radius: var(--r-pane, 28px);
    background: var(--paper);
    box-shadow: var(--shadow-card);
    overflow: hidden;
    animation: cardIn var(--dur-base) var(--ease-out);
  }
  @keyframes cardIn {
    from { opacity: 0; scale: var(--enter-scale); }
  }
  @media (prefers-reduced-motion: reduce) {
    .side-slot, .card { animation: none; }
  }
  .err-card {
    margin: auto;
    padding: var(--s-6);
    color: var(--danger);
  }

  /* Floating pills over content — no top bands (mockup & native-ui-study §2.1) */
  .float-bar {
    position: absolute;
    top: var(--s-3);
    left: var(--s-4);
    right: var(--s-4);
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    pointer-events: none;
  }
  .title-pill,
  .act-pill {
    pointer-events: auto;
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 38px;
    padding: 0 var(--s-3);
    border-radius: var(--r-pill);
    background: rgb(var(--tint) / 0.78);
    backdrop-filter: blur(var(--glass-blur, 16px)) saturate(var(--glass-sat, 180%));
    -webkit-backdrop-filter: blur(var(--glass-blur, 16px)) saturate(var(--glass-sat, 180%));
    box-shadow: var(--glass-edge), var(--shadow-float);
    max-width: min(75%, 24rem);
  }
  .pill-title {
    padding: 0 var(--s-3);
    font-size: var(--text-ui);
    font-weight: var(--strong-weight);
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pill-btn {
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--ink);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .pill-btn:hover { background: var(--hover); }
  .pill-btn:active { scale: var(--press-scale); }
  .pill-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  /* Bottom-centre floating strip (mockup AppStrip) */
  .strip {
    position: absolute;
    left: 50%;
    bottom: var(--s-2);
    translate: -50% 0;
    z-index: 10;
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    height: var(--tier-cap, 48px);
    padding: 0 var(--s-3);
    border-radius: var(--r-pill);
    background: rgb(var(--tint) / 0.78);
    backdrop-filter: blur(var(--glass-blur, 16px)) saturate(var(--glass-sat, 180%));
    -webkit-backdrop-filter: blur(var(--glass-blur, 16px)) saturate(var(--glass-sat, 180%));
    box-shadow: var(--glass-edge), var(--shadow-float);
  }
  .strip-btn {
    height: 36px;
    min-width: 36px;
    padding: 0 var(--s-3);
    border: 0;
    border-radius: var(--r-pill);
    background: transparent;
    color: var(--ink-2);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
    font: inherit;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .strip-btn:hover { background: var(--hover); color: var(--ink); }
  .strip-btn:active { scale: var(--press-scale); }
  .strip-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .strip-btn.cur { background: var(--accent-soft); color: var(--accent); }

  .host-shell.narrow .side-slot {
    position: fixed;
    inset: var(--inset, 8px) auto calc(var(--tier-cap, 48px) + var(--s-5)) var(--inset, 8px);
    width: min(82vw, 300px);
    z-index: 20;
  }
  .host-shell.narrow .pill-btn { width: 38px; height: 38px; }
  .host-shell.narrow .strip-btn { height: 40px; min-width: 40px; }
</style>
