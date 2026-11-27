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
    resolveReadPositionStateKey,
    rowTitle,
    type DerivedAction,
  } from "./format";
  import HostCollection from "./HostCollection.svelte";
  import HostFormSheet, { type FormFieldSpec } from "./HostFormSheet.svelte";
  import HostRecords, { type ActiveStreamRun } from "./HostRecords.svelte";
  import HostSidebar from "./HostSidebar.svelte";
  import {
    clearPersistedRun,
    loadLastOpenItem,
    loadPersistedRun,
    RunLogParser,
    saveLastOpenItem,
    savePersistedRun,
    type PersistedRun,
  } from "./runs";
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

  interface ItemRunState extends ActiveStreamRun {
    itemId: string;
    slot: string;
    filler: string;
    fillerOp: string;
    offset: number;
    turnDone: boolean;
    promptLink?: string;
    lastAnchorBefore?: string | null;
  }

  let activePluginId = $state<string | null>(null);
  let selectedCollectionId = $state<string | null>(null);
  let customSidebars = $state<{ id: string; name: string }[]>([]);
  let stripOrder = $state<string[]>([]);
  let collectionRows = $state<Record<string, RowData[]>>({});
  let collectionErrors = $state<Record<string, string>>({});
  let itemsById = $state<Record<string, RowData>>({});
  let recordsByItem = $state<Record<string, RecordData[]>>({});
  let itemErrors = $state<Record<string, string>>({});
  let runsByItem = $state<Record<string, ItemRunState>>({});

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
  const runSubs = new Map<string, () => void>();
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
        if (itemId in itemErrors) {
          const nextErr = { ...itemErrors };
          delete nextErr[itemId];
          itemErrors = nextErr;
        }
        recordsByItem = { ...recordsByItem, [itemId]: recs };
        applyLiveMeta(itemId, meta);
        maybeClearCompletedRun(itemId, recs);
      },
      onDelta: (recs, meta) => {
        if (itemId in itemErrors) {
          const nextErr = { ...itemErrors };
          delete nextErr[itemId];
          itemErrors = nextErr;
        }
        const cur = recordsByItem[itemId] ?? [];
        let next = cur;
        if (recs.length) {
          const seen = new Set(cur.map((r) => r.anchor));
          const fresh = recs.filter((r) => !seen.has(r.anchor));
          if (fresh.length) {
            next = [...cur, ...fresh];
            recordsByItem = { ...recordsByItem, [itemId]: next };
          }
        }
        applyLiveMeta(itemId, meta);
        maybeClearCompletedRun(itemId, next);
      },
      onAppend: (rec, version) => {
        const cur = recordsByItem[itemId] ?? [];
        const next = cur.some((r) => r.anchor === rec.anchor) ? cur : [...cur, rec];
        if (next !== cur) recordsByItem = { ...recordsByItem, [itemId]: next };
        const existing = itemsById[itemId];
        if (existing) itemsById = { ...itemsById, [itemId]: { ...existing, version } };
        maybeClearCompletedRun(itemId, next);
      },
      onItemChange: (meta) => applyLiveMeta(itemId, meta),
      onError: (_code, message) => {
        // Invalidate the affected item view so stale data is never shown as live.
        const nextRecs = { ...recordsByItem };
        delete nextRecs[itemId];
        recordsByItem = nextRecs;
        clearItemRun(itemId);
        itemErrors = { ...itemErrors, [itemId]: message || "This item is no longer available." };
      },
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

  function detachRunSub(itemId: string) {
    const unsub = runSubs.get(itemId);
    if (unsub) {
      unsub();
      runSubs.delete(itemId);
    }
  }

  function clearItemRun(itemId: string) {
    detachRunSub(itemId);
    if (session) clearPersistedRun(session.space, itemId);
    if (itemId in runsByItem) {
      const next = { ...runsByItem };
      delete next[itemId];
      runsByItem = next;
    }
  }

  function persistRunState(run: ItemRunState) {
    if (!session) return;
    if (run.status === "starting" || run.status === "streaming" || run.status === "stopping") {
      const payload: PersistedRun = {
        id: run.id,
        itemId: run.itemId,
        label: run.label,
        slot: run.slot,
        filler: run.filler,
        fillerOp: run.fillerOp,
        offset: run.offset,
        text: run.text,
        status: run.status,
        promptLink: run.promptLink,
      };
      savePersistedRun(session.space, payload);
    } else {
      clearPersistedRun(session.space, run.itemId);
    }
  }

  /** Replace the streaming bubble once the host's stored reply record carrying `run.id` arrives. */
  function maybeClearCompletedRun(itemId: string, recs: RecordData[]) {
    const run = runsByItem[itemId];
    if (!run || !run.id) return;
    if (run.status !== "streaming" && run.status !== "starting") return;
    if (recs.some((r) => r.run === run.id)) {
      clearItemRun(itemId);
    }
  }

  function attachRunLog(itemId: string, initial: ItemRunState) {
    if (!live || !initial.id) return;
    detachRunSub(itemId);
    const parser = new RunLogParser(initial.offset);
    runsByItem = { ...runsByItem, [itemId]: initial };
    persistRunState(initial);

    const unsub = live.subscribeRunLog(
      initial.id,
      {
        onBatch: (chunks, _offset, stopped) => {
          const cur = runsByItem[itemId];
          if (!cur || cur.id !== initial.id) return;
          const parsed = parser.feed(chunks);
          const nextText = parsed.textDelta ? cur.text + parsed.textDelta : cur.text;
          const turnDone = cur.turnDone || parsed.turnDone;
          let nextStatus = cur.status;
          if (parsed.textDelta && cur.status === "starting") {
            nextStatus = "streaming";
          }
          if (stopped || parsed.exited) {
            detachRunSub(itemId);
            if (cur.status === "stopping") {
              nextStatus = "stopped";
            } else if (turnDone) {
              // The reply turn completed; replace immediately if the run-stamped record is already here.
              const recs = recordsByItem[itemId] ?? [];
              if (recs.some((r) => r.run === cur.id)) {
                clearItemRun(itemId);
                return;
              }
              nextStatus = "streaming";
            } else {
              nextStatus = "lost";
            }
          }
          const updated: ItemRunState = {
            ...cur,
            text: nextText,
            offset: parsed.safeOffset,
            turnDone,
            status: nextStatus,
            canRetry: nextStatus === "stopped" ? Boolean(cur.fillerOp && cur.promptLink) : cur.canRetry,
          };
          runsByItem = { ...runsByItem, [itemId]: updated };
          persistRunState(updated);
        },
        onError: (code, message) => {
          const cur = runsByItem[itemId];
          if (!cur || cur.id !== initial.id) return;
          detachRunSub(itemId);
          const updated: ItemRunState = {
            ...cur,
            status: code === "not-found" || code === "conflict" ? "lost" : "failed",
            error: message,
            canRetry: false,
          };
          runsByItem = { ...runsByItem, [itemId]: updated };
          persistRunState(updated);
        },
      },
      initial.offset,
    );
    runSubs.set(itemId, unsub);
  }

  async function stopActiveRun(itemId: string) {
    const cur = runsByItem[itemId];
    if (!session || !cur || !cur.id || cur.status === "stopping") return;
    const updated: ItemRunState = { ...cur, status: "stopping" };
    runsByItem = { ...runsByItem, [itemId]: updated };
    persistRunState(updated);
    try {
      await LiveClient.stopRun(session, cur.id);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Could not stop run.";
      const latest = runsByItem[itemId];
      if (latest && latest.id === cur.id && latest.status === "stopping") {
        runsByItem = { ...runsByItem, [itemId]: { ...latest, status: "failed", error: msg, canRetry: false } };
      }
    }
  }

  async function retryRunStart(itemId: string) {
    const cur = runsByItem[itemId];
    if (!session || !cur || !cur.fillerOp || !cur.promptLink) return;
    const starting: ItemRunState = {
      ...cur,
      id: "",
      text: "",
      offset: 0,
      turnDone: false,
      status: "starting",
      error: undefined,
      canRetry: false,
    };
    runsByItem = { ...runsByItem, [itemId]: starting };
    const [fillerPid, fillerKid, fillerOpName] = cur.fillerOp.split(".");
    const fillerOpDecl = plugins.find((p) => p.id === fillerPid)?.kinds[fillerKid!]?.operations[fillerOpName!];
    const opInputs = Object.values(fillerOpDecl?.inputs ?? {});
    const itemKey =
      opInputs.find((f) => f.type.startsWith("link(") && !f.type.includes("records"))?.name ?? "item";
    const promptKey =
      opInputs.find((f) => f.type.startsWith("link(") && f.type.includes("records"))?.name ?? "prompt";
    try {
      const run = await hostCall<{ id: string; status: string }>(
        session,
        cur.fillerOp,
        { [itemKey]: itemId, [promptKey]: cur.promptLink },
        `retry-${crypto.randomUUID()}`,
      );
      if (run?.id) {
        attachRunLog(itemId, { ...starting, id: run.id, status: run.status === "stopped" ? "lost" : "starting" });
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Could not start the run.";
      runsByItem = {
        ...runsByItem,
        [itemId]: { ...starting, status: "failed", error: msg, canRetry: true },
      };
    }
  }

  function restorePersistedRunForItem(itemId: string) {
    if (!session || !live || runSubs.has(itemId) || runsByItem[itemId]) return;
    const saved = loadPersistedRun(session.space, itemId);
    if (!saved || !saved.id) return;
    attachRunLog(itemId, {
      id: saved.id,
      itemId: saved.itemId,
      label: saved.label,
      slot: saved.slot,
      filler: saved.filler,
      fillerOp: saved.fillerOp,
      offset: saved.offset,
      text: saved.text,
      status: saved.text ? "streaming" : "starting",
      turnDone: false,
      promptLink: saved.promptLink,
    });
  }

  function cleanupLive() {
    for (const unsub of runSubs.values()) unsub();
    runSubs.clear();
    runsByItem = {};
    itemErrors = {};
    collectionErrors = {};
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
      };

      const savedOpen = loadLastOpenItem(s.space);
      let restoredOpen = false;

      for (const p of decl.plugins) {
        for (const col of Object.values(p.collections)) {
          const cid = col.qualified;
          const unsub = live.subscribeQuery(
            { collection: cid },
            (rows) => {
              if (cid in collectionErrors) {
                const nextErr = { ...collectionErrors };
                delete nextErr[cid];
                collectionErrors = nextErr;
              }
              collectionRows = { ...collectionRows, [cid]: rows };
              const nextItems = { ...itemsById };
              for (const r of rows) nextItems[r.id] = r;
              itemsById = nextItems;
              if (savedOpen && !restoredOpen && nextItems[savedOpen]) {
                restoredOpen = true;
                wins.navigate("item", savedOpen);
                pushed = true;
                restorePersistedRunForItem(savedOpen);
              }
            },
            (_code, message) => {
              // Invalidate stale collection rows when the live query reports an error.
              const nextRows = { ...collectionRows };
              delete nextRows[cid];
              collectionRows = nextRows;
              collectionErrors = { ...collectionErrors, [cid]: message || "Could not load this collection." };
            },
          );
          unsubQueries.push(unsub);
        }
      }

      if (firstPlugin && !savedOpen) {
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
    if (session) saveLastOpenItem(session.space, row.id);
    if (wins.focused?.kind === "collection") {
      wins.navigate("item", row.id);
    } else {
      wins.openItemById(row.id);
    }
    pushed = true;
    restorePersistedRunForItem(row.id);
  }

  function openCollection(cid: string) {
    if (session) saveLastOpenItem(session.space, null);
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
    const picker = plugin ? resolvePickerSlot(plugin, comp?.slot.name) : null;
    if (!session || !comp) return;
    const qualifiedOp = `${kind.plugin}.${kind.id}.${comp.operation.name}`;
    const input: Record<string, unknown> = { item: item.id, [comp.textInput]: body };
    if (quoteAnchor && comp.quoteInput) input[comp.quoteInput] = quoteAnchor;
    const fillerObj = selectedFiller && picker ? picker.fillers.find((f) => f.plugin === selectedFiller) ?? null : null;
    const slotSelection = fillerObj && picker ? { slot: picker.slot.name, filler: fillerObj.plugin } : undefined;
    const fillerOp = fillerObj ? `${fillerObj.plugin}.${fillerObj.target}` : "";
    const curRecs = recordsByItem[item.id] ?? [];
    const lastAnchorBefore = curRecs.length ? curRecs[curRecs.length - 1]!.anchor : null;

    if (fillerObj && picker) {
      runsByItem = {
        ...runsByItem,
        [item.id]: {
          id: "",
          itemId: item.id,
          label: fillerObj.label,
          slot: picker.slot.name,
          filler: fillerObj.plugin,
          fillerOp,
          offset: 0,
          text: "",
          status: "starting",
          turnDone: false,
          lastAnchorBefore,
        },
      };
    }

    try {
      const res = await hostCall<{ changes?: { link?: string; anchor?: string }[]; run?: { id: string; status: string } }>(
        session,
        qualifiedOp,
        input,
        `send-${crypto.randomUUID()}`,
        slotSelection,
      );
      if (fillerObj && picker && res?.run?.id) {
        const promptLink = res.changes?.[0]?.link;
        const promptAnchor = res.changes?.[0]?.anchor ?? lastAnchorBefore;
        if (res.run.status === "stopped") {
          runsByItem = {
            ...runsByItem,
            [item.id]: {
              id: res.run.id,
              itemId: item.id,
              label: fillerObj.label,
              slot: picker.slot.name,
              filler: fillerObj.plugin,
              fillerOp,
              offset: 0,
              text: "",
              status: "lost",
              turnDone: false,
              promptLink,
              lastAnchorBefore: promptAnchor,
            },
          };
        } else {
          attachRunLog(item.id, {
            id: res.run.id,
            itemId: item.id,
            label: fillerObj.label,
            slot: picker.slot.name,
            filler: fillerObj.plugin,
            fillerOp,
            offset: 0,
            text: "",
            status: "starting",
            turnDone: false,
            promptLink,
            lastAnchorBefore: promptAnchor,
          });
        }
      }
    } catch (err) {
      if (fillerObj && picker) {
        const msg = err instanceof Error ? err.message : "Could not start the run.";
        const latestRecs = recordsByItem[item.id] ?? [];
        const latestAnchor = latestRecs.length ? latestRecs[latestRecs.length - 1]!.anchor : null;
        const messageWasAppended = latestAnchor !== null && latestAnchor !== lastAnchorBefore;
        runsByItem = {
          ...runsByItem,
          [item.id]: {
            id: "",
            itemId: item.id,
            label: fillerObj.label,
            slot: picker.slot.name,
            filler: fillerObj.plugin,
            fillerOp,
            offset: 0,
            text: "",
            status: "failed",
            turnDone: false,
            error: msg,
            canRetry: messageWasAppended,
            promptLink: messageWasAppended ? `[Record](<#^${latestAnchor}>)` : undefined,
            lastAnchorBefore: latestAnchor,
          },
        };
        if (messageWasAppended) return;
      }
      throw err;
    }
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
              {#if collectionErrors[w.ref]}
                <p class="err" role="alert">{collectionErrors[w.ref]}</p>
              {:else if col}
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
              {@const picker = plugin ? resolvePickerSlot(plugin, comp?.slot.name) : null}
              {@const readStateKey = kind ? resolveReadPositionStateKey(kind) : null}
              {#if itemErrors[w.ref]}
                <p class="err" role="alert">{itemErrors[w.ref]}</p>
              {:else if row && kind}
                <HostRecords
                  {row}
                  {kind}
                  records={recordsByItem[row.id] ?? []}
                  viewerId={session?.account ?? ""}
                  composer={comp}
                  pickerSlot={picker}
                  activeRun={runsByItem[row.id] ?? null}
                  onsend={(body, quote, filler) => sendRecord(row, kind, body, quote, filler)}
                  onstoprun={() => stopActiveRun(row.id)}
                  onretryrun={() => retryRunStart(row.id)}
                  ondismissrun={() => clearItemRun(row.id)}
                  onmarkread={readStateKey
                    ? (anchor) => {
                        if (!session || row.state?.[readStateKey] === anchor) return;
                        void hostCall(session, "s.state", { item: row.id, [readStateKey]: anchor }).catch(() => {});
                      }
                    : undefined}
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
