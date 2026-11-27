<script lang="ts">
  /**
   * Generic records view, composer slot with person-picked secondary slot filler picker,
   * and growing streamed reply + Stop control surface.
   * Drawn strictly from `s.declarations` with zero hardcoded plugin vocabulary.
   */
  import { onMount, tick } from "svelte";
  import { Icon, Menu, type MenuItem } from "@calternal/ui";
  import {
    dayKey,
    dayLabel,
    formatClock,
    humanize,
    initials,
    type ResolvedComposer,
    type ResolvedPickerSlot,
  } from "./format";
  import type { KindDecl, RecordData, RowData } from "./types";

  /** Real agent run lifecycle states rendered with motion in the transcript. */
  export type RunUiState = "starting" | "streaming" | "stopping" | "stopped" | "failed" | "lost";

  /** Active or recently-terminated run state drawn in the records view. */
  export interface ActiveStreamRun {
    id: string;
    label: string;
    text: string;
    status: RunUiState;
    error?: string;
    canRetry?: boolean;
  }

  let {
    row,
    kind,
    records,
    viewerId,
    composer,
    pickerSlot,
    activeRun = null,
    onsend,
    onstoprun,
    onretryrun,
    ondismissrun,
    onmarkread,
    oneditrecord,
    onremoverecord,
  }: {
    row: RowData;
    kind: KindDecl;
    records: RecordData[];
    viewerId: string;
    composer: ResolvedComposer | null;
    pickerSlot: ResolvedPickerSlot | null;
    activeRun?: ActiveStreamRun | null;
    onsend: (body: string, quoteAnchor?: string, selectedFiller?: string | null) => Promise<void>;
    onstoprun?: () => void;
    onretryrun?: () => void;
    ondismissrun?: () => void;
    onmarkread?: (anchor: string) => void;
    oneditrecord: (rec: RecordData) => void;
    onremoverecord: (rec: RecordData) => void;
  } = $props();

  const canQuote = $derived(Boolean(composer?.quoteInput && composer.quoteInput in kind.records));
  const canEdit = $derived("change record" in kind.operations);
  const canRemove = $derived("remove record" in kind.operations);
  const sendLabel = $derived(composer ? humanize(composer.operation.name) : "Send");
  const composerLabel = $derived(composer?.label || "Write");
  const runIsActive = $derived(
    activeRun !== null &&
      (activeRun.status === "starting" || activeRun.status === "streaming" || activeRun.status === "stopping"),
  );
  const runStatusText = $derived.by(() => {
    if (!activeRun) return "";
    switch (activeRun.status) {
      case "starting":
        return "Starting…";
      case "streaming":
        return "Replying…";
      case "stopping":
        return "Stopping…";
      case "stopped":
        return "Stopped by you";
      case "failed":
        return "Failed to start";
      case "lost":
        return "Run lost after restart";
    }
  });

  const own = (r: RecordData) => !r.run && r.author === viewerId;
  const displayName = (r: RecordData) => r.author_name || (r.run ? "Agent" : r.author);

  type Line =
    | { t: "day"; key: string; text: string }
    | { t: "rec"; key: string; r: RecordData; first: boolean; last: boolean };

  const lines = $derived.by(() => {
    const out: Line[] = [];
    const shown = records.filter((r) => !r.redacted);
    let prevDay = "";
    shown.forEach((r, i) => {
      const d = dayKey(r.at);
      const p = shown[i - 1];
      const n = shown[i + 1];
      if (d && d !== prevDay) {
        out.push({ t: "day", key: `d:${d}`, text: dayLabel(r.at) });
        prevDay = d;
      }
      const near = (a?: RecordData, b?: RecordData) =>
        !!a &&
        !!b &&
        a.author === b.author &&
        dayKey(a.at) === dayKey(b.at) &&
        Math.abs(Date.parse(b.at) - Date.parse(a.at)) < 300_000;
      out.push({
        t: "rec",
        key: r.anchor,
        r,
        first: !near(p, r),
        last: !near(r, n),
      });
    });
    return out;
  });

  const byAnchor = $derived(new Map(records.map((r) => [r.anchor, r])));

  let menu = $state<{ r: RecordData; el: HTMLElement } | null>(null);
  const menuItems = $derived.by<MenuItem[]>(() => {
    if (!menu) return [];
    const items: MenuItem[] = [];
    if (canQuote) items.push({ id: "quote", label: "Quote", icon: "quote" });
    if (canEdit && own(menu.r)) items.push({ id: "edit", label: "Edit" });
    if (canRemove && own(menu.r)) items.push({ id: "remove", label: "Remove", icon: "trash", danger: true });
    return items;
  });

  // Secondary slot filler picker (e.g. harness slot) drawn strictly from `s.declarations`.
  let selectedFiller = $state<string | null>(null);
  let pickerEl = $state<HTMLElement | null>(null);
  let pickerOpen = $state(false);
  const selectedFillerObj = $derived(
    pickerSlot?.fillers.find((f) => f.plugin === selectedFiller) ?? null,
  );
  const pickerItems = $derived<MenuItem[]>(
    pickerSlot
      ? [
          ...pickerSlot.fillers.map((f) => ({ id: f.plugin, label: f.label })),
          ...(selectedFiller ? [{ id: "__none__", label: "None" }] : []),
        ]
      : [],
  );

  let quoting = $state<{ anchor: string; author: string; excerpt: string } | null>(null);
  let draft = $state("");
  let sending = $state(false);
  let sendError = $state("");
  let scroller = $state<HTMLElement>();
  let atEnd = $state(true);

  const draftKey = $derived(
    composer?.draftState ? `calternal.host.${composer.draftState}.${row.id}` : null,
  );
  let lastDraftKey: string | null = null;
  $effect(() => {
    const key = draftKey;
    if (key === lastDraftKey) return;
    lastDraftKey = key;
    if (!key) {
      draft = "";
    } else {
      try {
        const stored = localStorage.getItem(key);
        const stateDraft = composer?.draftState ? row.state?.[composer.draftState] : undefined;
        draft = stored ?? (typeof stateDraft === "string" ? stateDraft : "");
      } catch {
        draft = "";
      }
    }
    quoting = null;
    sendError = "";
  });

  function saveDraft() {
    if (!draftKey) return;
    try {
      if (draft) localStorage.setItem(draftKey, draft);
      else localStorage.removeItem(draftKey);
    } catch {
      // ignore storage errors
    }
  }

  function pickRecordAction(id: string) {
    const cur = menu;
    menu = null;
    if (!cur) return;
    if (id === "quote") {
      quoting = {
        anchor: cur.r.anchor,
        author: displayName(cur.r),
        excerpt: cur.r.body.split("\n")[0] ?? "",
      };
      return;
    }
    if (id === "edit") oneditrecord(cur.r);
    if (id === "remove") onremoverecord(cur.r);
  }

  function updateAtEnd() {
    if (!scroller) return;
    atEnd = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 48;
  }

  function scrollToEnd(instant = false) {
    if (!scroller) return;
    const reduce = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
    scroller.scrollTo({
      top: scroller.scrollHeight,
      behavior: instant || reduce ? "auto" : "smooth",
    });
  }

  // Keep pinned to the bottom when new records arrive or streamed text grows while near the end.
  let prevLen = 0;
  let prevStreamLen = 0;
  $effect(() => {
    const len = records.length;
    const sLen = activeRun?.text.length ?? 0;
    if ((len > prevLen || sLen > prevStreamLen) && atEnd) {
      void tick().then(() => scrollToEnd(prevLen === 0 && prevStreamLen === 0));
    }
    prevLen = len;
    prevStreamLen = sLen;
  });

  // Debounced read-position acknowledgement when scrolled to the bottom and the page is visible.
  let lastMarkedAnchor = "";
  let markTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    void row.id;
    lastMarkedAnchor = "";
  });
  $effect(() => {
    if (!onmarkread || !atEnd) return;
    if (typeof document !== "undefined" && document.visibilityState !== "visible") return;
    const shown = records.filter((r) => !r.redacted);
    const tail = shown.length ? shown[shown.length - 1]!.anchor : "";
    if (!tail || tail === lastMarkedAnchor) return;
    if (markTimer) clearTimeout(markTimer);
    markTimer = setTimeout(() => {
      if (typeof document !== "undefined" && document.visibilityState !== "visible") return;
      lastMarkedAnchor = tail;
      onmarkread(tail);
    }, 350);
    return () => {
      if (markTimer) { clearTimeout(markTimer); markTimer = null; }
    };
  });

  onMount(() => {
    void tick().then(() => {
      scrollToEnd(true);
      updateAtEnd();
    });
    const onVis = () => {
      if (document.visibilityState === "visible" && atEnd && onmarkread) {
        const shown = records.filter((r) => !r.redacted);
        const tail = shown.length ? shown[shown.length - 1]!.anchor : "";
        if (tail && tail !== lastMarkedAnchor) {
          lastMarkedAnchor = tail;
          onmarkread(tail);
        }
      }
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  });

  async function submitComposer(e: Event) {
    e.preventDefault();
    const text = draft.trim();
    if (!text || sending) return;
    sending = true;
    sendError = "";
    const quoteAnchor = quoting?.anchor;
    const filler = selectedFiller;
    draft = "";
    quoting = null;
    saveDraft();
    try {
      await onsend(text, quoteAnchor, filler);
    } catch (err) {
      draft = text;
      sendError = err instanceof Error ? err.message : "Could not send.";
    } finally {
      sending = false;
    }
  }
</script>

<section class="cv" aria-label="Records">
  <div
    class="tr"
    bind:this={scroller}
    onscroll={updateAtEnd}
    role="log"
    aria-live="polite"
    aria-label="Records"
  >
    {#if !lines.length && !activeRun}
      <p class="empty">No records yet.</p>
    {/if}
    {#each lines as l (l.key)}
      {#if l.t === "day"}
        <div class="mk"><time>{l.text}</time></div>
      {:else}
        {@const r = l.r}
        {@const quoteRef = typeof r.fields?.quote === "string" ? byAnchor.get(r.fields.quote) : undefined}
        <article
          class="e"
          class:own={own(r)}
          class:first={l.first}
          class:last={l.last}
          data-record-anchor={r.anchor}
          aria-label={`${displayName(r)}, ${formatClock(r.at)}`}
          oncontextmenu={(e) => {
            if (!canQuote && !canEdit && !canRemove) return;
            e.preventDefault();
            menu = { r, el: e.currentTarget as HTMLElement };
          }}
        >
          {#if l.first && !own(r)}
            <header class="eh">
              <span class="av" aria-hidden="true">{initials(displayName(r))}</span>
              <span class="nm">{displayName(r)}</span>
            </header>
          {/if}
          <div class="bub">
            {#if quoteRef}
              <button
                type="button"
                class="qt"
                onclick={() =>
                  scroller
                    ?.querySelector(`[data-record-anchor="${quoteRef.anchor}"]`)
                    ?.scrollIntoView({ block: "center", behavior: "smooth" })}
              >
                <span class="qa">{displayName(quoteRef)}</span>
                <span class="qx">{quoteRef.body}</span>
              </button>
            {/if}
            <p class="body">{r.body}</p>
            <span class="meta">
              {#if r.edited}<span class="ed">edited</span>{/if}
              <time>{formatClock(r.at)}</time>
            </span>
          </div>
          {#if canQuote || (own(r) && (canEdit || canRemove))}
            <button
              type="button"
              class="more"
              aria-label="Record actions"
              onclick={(e) => (menu = { r, el: e.currentTarget as HTMLElement })}
            >
              <Icon name="more" inline />
            </button>
          {/if}
        </article>
      {/if}
    {/each}

    {#if activeRun}
      <article
        class="e first last streaming"
        class:run-failed={activeRun.status === "failed"}
        class:run-lost={activeRun.status === "lost"}
        class:run-stopped={activeRun.status === "stopped"}
        data-stream-run={activeRun.id}
        data-run-state={activeRun.status}
        aria-label={`${activeRun.label}, ${runStatusText}`}
        aria-busy={runIsActive ? "true" : "false"}
      >
        <header class="eh">
          <span class="pulse-dot" class:idle={!runIsActive} aria-hidden="true"></span>
          <span class="nm">{activeRun.label}</span>
          <span class="stream-state">{runStatusText}</span>
        </header>
        <div class="bub stream-bub">
          {#if activeRun.text}
            <p class="body">{activeRun.text}</p>
          {:else if activeRun.status === "failed"}
            <p class="body run-note">{activeRun.error || "Could not start the run."}</p>
          {:else if activeRun.status === "lost"}
            <p class="body run-note">The host restarted before this reply finished.</p>
          {:else if activeRun.status === "stopped"}
            <p class="body run-note">Stopped before any reply was written.</p>
          {:else}
            <p class="body">…</p>
          {/if}
          {#if !runIsActive}
            <div class="run-actions">
              {#if (activeRun.status === "failed" || activeRun.status === "stopped") && activeRun.canRetry && onretryrun}
                <button type="button" class="run-act primary" onclick={onretryrun}>Retry</button>
              {/if}
              {#if ondismissrun}
                <button type="button" class="run-act" onclick={ondismissrun}>Dismiss</button>
              {/if}
            </div>
          {/if}
        </div>
      </article>
    {/if}
  </div>

  {#if !atEnd}
    <button
      type="button"
      class="jb"
      aria-label="Jump to latest"
      onclick={() => scrollToEnd(false)}
    >
      <Icon name="down" inline />
    </button>
  {/if}

  {#if composer}
    <form class="cp" onsubmit={submitComposer} aria-label="Composer">
      {#if quoting}
        <div class="q">
          <span class="qa">{quoting.author}</span>
          <span class="qx">{quoting.excerpt}</span>
          <button
            type="button"
            class="x"
            aria-label="Remove quote"
            onclick={() => (quoting = null)}
          >
            <Icon name="close" inline />
          </button>
        </div>
      {/if}
      {#if sendError}
        <p class="cerr" role="alert">{sendError}</p>
      {/if}
      <div class="crow">
        <textarea
          rows="1"
          bind:value={draft}
          oninput={saveDraft}
          placeholder={composerLabel}
          aria-label={composerLabel}
          onkeydown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) void submitComposer(e);
          }}
        ></textarea>
        {#if pickerSlot}
          {@const slotLabel = pickerSlot.label}
          <button
            type="button"
            class="slot-btn"
            class:picked={!!selectedFillerObj}
            bind:this={pickerEl}
            data-slot-picker={pickerSlot.slot.name}
            data-selected-filler={selectedFillerObj?.plugin ?? ""}
            aria-label={selectedFillerObj
              ? `${slotLabel}: ${selectedFillerObj.label}`
              : slotLabel}
            aria-expanded={pickerOpen}
            onclick={() => (pickerOpen = !pickerOpen)}
          >
            <Icon name="spark" inline />
            <span class="slot-lbl">
              {selectedFillerObj ? selectedFillerObj.label : slotLabel}
            </span>
          </button>
        {/if}
        {#if runIsActive && onstoprun}
          <button
            type="button"
            class="stop"
            disabled={activeRun?.status === "stopping" || !activeRun?.id}
            aria-label="Stop"
            onclick={onstoprun}
          >
            <Icon name="stop-round" inline />
          </button>
        {:else}
          <button
            type="submit"
            class="send"
            disabled={!draft.trim() || sending}
            aria-label={sendLabel}
          >
            <Icon name="send" inline />
          </button>
        {/if}
      </div>
    </form>
  {/if}

  <Menu
    open={menu !== null}
    items={menuItems}
    label="Record actions"
    onpick={pickRecordAction}
    onclose={() => (menu = null)}
    trigger={menu?.el ?? null}
  />

  {#if pickerSlot}
    <Menu
      open={pickerOpen}
      items={pickerItems}
      label={humanize(pickerSlot.slot.name)}
      onpick={(id) => {
        pickerOpen = false;
        selectedFiller = id === "__none__" ? null : id;
      }}
      onclose={() => (pickerOpen = false)}
      trigger={pickerEl}
    />
  {/if}
</section>

<style>
  .cv {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    height: 100%;
    overflow: hidden;
  }
  .tr {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: column;
    padding: var(--s-2) var(--s-6) var(--s-4);
  }
  .empty {
    margin: auto;
    color: var(--ink-2);
    font-size: var(--text-ui);
    text-align: center;
  }
  .mk {
    align-self: center;
    margin-block: var(--s-4) var(--s-2);
    color: var(--ink-2);
    font-size: var(--text-label);
  }
  .mk time {
    display: inline-block;
    padding: var(--s-2) var(--s-5);
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.07);
  }
  .e {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--s-1);
    margin-top: var(--s-1);
    max-width: min(82%, 38rem);
    position: relative;
    animation: rise var(--dur-base) var(--ease-out);
  }
  .e.first { margin-top: var(--s-5); }
  .e.own { align-self: flex-end; align-items: flex-end; }
  .stream-bub {
    border: 1px solid color-mix(in oklab, var(--accent) 28%, transparent);
    transition: height var(--dur-fast) var(--ease-out), opacity var(--dur-fast) var(--ease-out), border-color var(--dur-base) var(--ease-out);
  }
  .run-failed .stream-bub { border-color: color-mix(in oklab, var(--danger) 45%, transparent); }
  .run-lost .stream-bub, .run-stopped .stream-bub { border-color: rgb(var(--ink-rgb) / 0.16); }
  .pulse-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse var(--dur-pulse, 1800ms) var(--ease-out) infinite;
  }
  .pulse-dot.idle { animation: none; opacity: 0.45; }
  .run-failed .pulse-dot { background: var(--danger); opacity: 0.85; }
  .stream-state {
    color: var(--ink-2);
    font-size: var(--text-label);
  }
  .run-note { color: var(--ink-2); font-style: italic; }
  .run-actions { display: flex; gap: var(--s-3); margin-top: var(--s-2); }
  .run-act {
    min-height: 28px;
    padding: 0 var(--s-4);
    border: 0;
    border-radius: var(--r-pill);
    background: var(--hover);
    color: var(--ink);
    font: inherit;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .run-act.primary { background: var(--accent-soft); color: var(--accent); }
  .run-act:hover { background: rgb(var(--ink-rgb) / 0.12); }
  .run-act:active { scale: var(--press-scale); }
  .run-act:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @keyframes rise {
    from { opacity: 0; translate: 0 6px; }
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; scale: 1; }
    50% { opacity: 0.4; scale: 0.85; }
  }
  @media (prefers-reduced-motion: reduce) {
    .e, .pulse-dot { animation: none; }
    .stream-bub { transition: none; }
  }
  .eh {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding-inline: var(--s-3);
    font-size: var(--text-label);
    color: var(--ink-2);
  }
  .av {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: inline-grid;
    place-items: center;
    background: color-mix(in oklab, var(--accent-base) 20%, transparent);
    color: var(--accent);
    font-size: 9px;
    font-weight: var(--strong-weight);
  }
  .nm { font-weight: var(--strong-weight); color: var(--ink); }

  .bub {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--s-3);
    padding: var(--s-4) var(--s-5) var(--s-6);
    border-radius: var(--r-card);
    background: rgb(var(--ink-rgb) / 0.07);
    min-width: 7rem;
    max-width: 100%;
    box-sizing: border-box;
  }
  .e:not(.own):not(.first) .bub { border-top-left-radius: var(--r-select); }
  .e:not(.own):not(.last) .bub { border-bottom-left-radius: var(--r-select); }
  .e.own .bub { background: var(--accent-soft); }
  .e.own:not(.first) .bub { border-top-right-radius: var(--r-select); }
  .e.own:not(.last) .bub { border-bottom-right-radius: var(--r-select); }

  .body {
    margin: 0;
    font-size: var(--text-ui);
    line-height: 1.45;
    color: var(--ink);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .qt {
    position: relative;
    display: grid;
    gap: 2px;
    align-self: stretch;
    border: 0;
    text-align: start;
    padding: var(--s-2) var(--s-4) var(--s-2) var(--s-5);
    border-radius: 0 var(--r-select) var(--r-select) 0;
    background: color-mix(in oklab, var(--accent) 12%, transparent);
    cursor: pointer;
  }
  .qt::before {
    content: "";
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    width: 3px;
    background: var(--accent);
  }
  .qa { font-weight: var(--strong-weight); font-size: var(--text-label); color: var(--accent); }
  .qx {
    color: var(--ink-2);
    font-size: var(--text-label);
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  /* High-contrast timestamp in both light and dark themes (matches mockup Conversation.svelte) */
  .meta {
    position: absolute;
    right: var(--s-5);
    bottom: var(--s-2);
    display: inline-flex;
    gap: var(--s-3);
    color: var(--ink-2);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .ed { font-style: italic; }

  .more {
    position: absolute;
    top: 50%;
    translate: 0 -50%;
    inset-inline-end: -32px;
    width: 28px;
    height: 28px;
    border: 0;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: transparent;
    color: var(--ink-2);
    opacity: 0;
    cursor: pointer;
    transition: opacity var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }
  .e.own .more { inset-inline-end: auto; inset-inline-start: -32px; }
  .e:hover .more, .more:focus-visible { opacity: 1; }
  .more:hover { background: var(--hover); color: var(--ink); }
  .more:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (hover: none) { .more { display: none; } }

  .jb {
    position: absolute;
    right: var(--s-6);
    bottom: 92px;
    width: 36px;
    height: 36px;
    border: 0;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--paper);
    color: var(--ink);
    box-shadow: var(--shadow-float);
    cursor: pointer;
    transition: scale var(--dur-fast) var(--ease-out);
  }
  .jb:active { scale: var(--press-scale); }
  .jb:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .cp {
    flex: none;
    display: grid;
    gap: var(--s-3);
    margin: 0 var(--s-5) var(--s-5);
    padding: var(--s-3);
    border-radius: var(--r-card);
    background: rgb(var(--ink-rgb) / 0.07);
  }
  .q {
    position: relative;
    display: grid;
    gap: 2px;
    padding: var(--s-2) var(--s-8) var(--s-2) var(--s-5);
    background: color-mix(in oklab, var(--accent) 12%, transparent);
    border-radius: 0 var(--r-select) var(--r-select) 0;
  }
  .q::before {
    content: "";
    position: absolute;
    inset-block: 0;
    left: 0;
    width: 3px;
    background: var(--accent);
  }
  .x {
    position: absolute;
    right: var(--s-2);
    top: 50%;
    translate: 0 -50%;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--ink-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .cerr { margin: 0 var(--s-3); color: var(--danger); font-size: var(--text-label); }
  .crow { display: flex; align-items: flex-end; gap: var(--s-3); }

  .slot-btn {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 34px;
    padding: 0 var(--s-4);
    border: 0;
    border-radius: var(--r-pill);
    background: var(--hover);
    color: var(--ink-2);
    font: inherit;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .slot-btn:hover { background: rgb(var(--ink-rgb) / 0.12); color: var(--ink); }
  .slot-btn:active { scale: var(--press-scale); }
  .slot-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .slot-btn.picked {
    background: var(--accent-soft);
    color: var(--accent);
  }
  @media (max-width: 520px) {
    .slot-btn:not(.picked) {
      width: 34px;
      padding: 0;
      justify-content: center;
    }
    .slot-btn:not(.picked) .slot-lbl { display: none; }
  }

  textarea {
    flex: 1;
    min-width: 0;
    min-height: 34px;
    max-height: 8rem;
    padding: var(--s-3) var(--s-3);
    border: 0;
    outline: 0;
    resize: none;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: var(--text-ui);
    line-height: 1.35;
  }
  .send,
  .stop {
    flex: none;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border: 0;
    border-radius: 50%;
    cursor: pointer;
    transition: scale var(--dur-fast) var(--ease-out), opacity var(--dur-fast) var(--ease-out);
  }
  .send {
    background: var(--accent-base);
    color: #fff;
  }
  .stop {
    background: var(--danger);
    color: #fff;
  }
  .send:active, .stop:active { scale: var(--press-scale); }
  .send:disabled, .stop:disabled { opacity: 0.4; cursor: default; }
  .send:focus-visible, .stop:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  @media (max-width: 759px) {
    .tr { padding-inline: var(--s-4); }
    .e { max-width: 88%; }
    .cp { margin: 0 var(--s-4) var(--s-4); }
    .send, .stop { width: 40px; height: 40px; }
    .slot-btn { min-height: 40px; padding: 0 var(--s-3); }
    .slot-lbl { max-width: 7rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  }
</style>
