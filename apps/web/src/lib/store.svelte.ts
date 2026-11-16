import * as api from './api';
import { ApiError, describeError } from './api';
import { parsePath, hrefFor, type Route } from './route';
import type { Describe, Note, NoteSummary, NoteVersion, PluginEntry, Role, SummarizeResult, GenUiView } from './types';

export interface ActivityItem { id: number; at: string; kind: 'info' | 'success' | 'warning' | 'error'; text: string; href?: string }
type Conn = 'disconnected' | 'connecting' | 'connected';

class AppStore {
  conn = $state<Conn>('disconnected');
  connError = $state<string | null>(null);
  info = $state<Describe | null>(null);
  route = $state<Route>(parsePath(location.pathname));
  navOpen = $state(false);
  aiOpen = $state(false);
  activity = $state<ActivityItem[]>([]);
  lastSpace = $state<string | null>(null);
  #actId = 0;
  /** Bumped on every connect/disconnect. Async results from an older epoch are dropped, never applied. */
  #epoch = 0;

  // notes
  notes = $state<NoteSummary[]>([]);
  notesState = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  notesError = $state<string | null>(null);
  notesSpace = '';

  note = $state<Note | null>(null);
  noteState = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  noteError = $state<{ code: string; message: string } | null>(null);
  draftTitle = $state('');
  draftMarkdown = $state('');
  saving = $state(false);
  saveError = $state<string | null>(null);
  /** server copy fetched after a conflict, shown beside the preserved draft */
  conflict = $state<Note | null>(null);
  #pendingSave: { key: string; requestId: string } | null = null;
  /** Unsaved drafts kept in memory per resource so navigation/back/forward never loses edits. */
  drafts = $state<Record<string, { base: Note | null; title: string; markdown: string }>>({});
  draftKey(space: string, id: string) { return `${space}/${id}`; }
  hasDraft(space: string, id: string) { return !!this.drafts[this.draftKey(space, id)]; }
  /** Stash the current editor state if it differs from its base; call before it is replaced. */
  stashDraft() {
    const n = this.note;
    if (n) {
      const k = this.draftKey(n.space_id, n.id);
      if (this.dirty) this.drafts[k] = { base: n, title: this.draftTitle, markdown: this.draftMarkdown };
      else delete this.drafts[k];
    } else if (this.route.name === 'notes' && this.route.view === 'new' && (this.draftTitle || this.draftMarkdown)) {
      this.drafts[this.draftKey(this.route.space, 'new')] = { base: null, title: this.draftTitle, markdown: this.draftMarkdown };
    }
  }

  history = $state<NoteVersion[]>([]);
  historyState = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  historyError = $state<string | null>(null);
  historyNext = $state<string | null>(null);

  plugins = $state<PluginEntry[]>([]);
  pluginsState = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  pluginsError = $state<string | null>(null);
  pluginsSpace = '';
  pluginBusy = $state<string | null>(null);

  insights = $state<{ noteId: string; revision: string; result: SummarizeResult } | null>(null);
  insightsBusy = $state(false);
  insightsError = $state<string | null>(null);

  constructor() {
    window.addEventListener('popstate', () => this.applyLocation());
    // Client-side navigation for same-origin in-app links (plain paths, History API).
    window.addEventListener('click', (e) => {
      if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      const a = (e.target as Element | null)?.closest?.('a[href]') as HTMLAnchorElement | null;
      if (!a || a.target || a.origin !== location.origin || a.pathname.startsWith('/api/') || a.getAttribute('href')?.startsWith('#')) return;
      e.preventDefault();
      this.navigate(a.pathname);
    });
    window.addEventListener('beforeunload', (e) => { if (this.dirty || Object.keys(this.drafts).length) { e.preventDefault(); } });
  }

  // ---- derived ----
  get spaces() { return this.info?.principal.spaces ?? []; }
  get dirty() {
    return !!this.note && (this.draftTitle !== this.note.title || this.draftMarkdown !== this.note.markdown);
  }
  get currentSpace(): string | null {
    const r = this.route;
    if (r.name === 'notes' || r.name === 'plugins') return r.space;
    return this.lastSpace ?? this.spaces[0]?.id ?? null;
  }
  roleIn(space: string | null): Role | null {
    return this.spaces.find((s) => s.id === space)?.role ?? null;
  }
  get canEdit() { const r = this.roleIn(this.currentSpace); return r === 'editor' || r === 'manager'; }
  get canManage() { return this.roleIn(this.currentSpace) === 'manager'; }
  hasOp(id: string) { return !!this.info?.operations.some((o) => o.id === id); }

  log(kind: ActivityItem['kind'], text: string, href?: string) {
    this.activity = [{ id: ++this.#actId, at: new Date().toISOString(), kind, text, href }, ...this.activity].slice(0, 100);
  }

  private applyLocation() { this.stashDraft(); this.route = parsePath(location.pathname); this.navOpen = false; this.syncRoute(); }
  navigate(path: string, replace = false) {
    if (path === location.pathname) { this.navOpen = false; return; }
    if (replace) history.replaceState(null, '', path); else history.pushState(null, '', path);
    this.applyLocation();
  }
  go(r: Route, replace = false) { this.navigate(hrefFor(r), replace); }

  /** Returns false when the user cancels leaving unsaved edits. */
  confirmLeave(): boolean {
    this.stashDraft();
    return !Object.keys(this.drafts).length || confirm('Unsaved drafts are kept only in this tab’s memory. Disconnecting discards them. Continue?');
  }

  // ---- connection ----
  async connect(token: string) {
    api.setToken(token);
    const ep = ++this.#epoch;
    this.conn = 'connecting'; this.connError = null;
    this.resetData();
    try {
      const info = await api.describe();
      if (ep !== this.#epoch) return;
      this.info = info;
      this.conn = 'connected';
      this.log('success', `Connected as ${this.info.principal.id}`);
      if (this.route.name === 'home') {
        const first = this.info.principal.spaces[0];
        if (first) this.go({ name: 'notes', space: first.id }, true);
      }
      this.syncRoute();
    } catch (e) {
      if (ep !== this.#epoch) return;
      api.setToken(null);
      this.conn = 'disconnected';
      this.connError = describeError(e);
      this.info = null;
    }
  }

  disconnect() {
    this.#epoch++;
    api.setToken(null);
    this.resetData();
    this.info = null; this.conn = 'disconnected'; this.connError = null; this.activity = [];
  }

  /** Clear everything derived from the previous principal. */
  resetData() {
    this.notes = []; this.notesState = 'idle'; this.notesSpace = '';
    this.note = null; this.noteState = 'idle'; this.draftTitle = ''; this.draftMarkdown = '';
    this.conflict = null; this.saveError = null; this.#pendingSave = null;
    this.history = []; this.historyState = 'idle'; this.plugins = []; this.pluginsState = 'idle'; this.pluginsSpace = '';
    this.insights = null; this.insightsError = null; this.drafts = {}; this.lastSpace = null; this.saving = false;
    this.insightsBusy = false; this.pluginBusy = null; this.historyNext = null; this.draftTitle = ''; this.draftMarkdown = '';
  }

  private handleAuth(e: unknown, ep = this.#epoch) {
    if (ep === this.#epoch && e instanceof ApiError && e.code === 'unauthorized') {
      this.disconnect();
      this.connError = 'The server rejected the token. Enter it again.';
    }
  }

  // ---- routing sync ----
  syncRoute() {
    if (this.conn !== 'connected') return;
    const r = this.route;
    if (r.name === 'notes' || r.name === 'plugins') {
      this.lastSpace = r.space;
      if (!this.roleIn(r.space)) return;
    }
    if (r.name === 'notes') {
      if (r.view === 'new') { const d = this.drafts[this.draftKey(r.space, 'new')]; this.note = null; this.noteState = 'ready'; this.draftTitle = d?.title ?? ''; this.draftMarkdown = d?.markdown ?? ''; this.resetSaveState(); }
      void this.loadNotes(r.space);
      if (this.hasOp('plugins.list') || !this.info?.operations.length) void this.loadPlugins(r.space); // extension panels need the enabled-plugin list
      if (r.id) void this.openNote(r.space, r.id, r.view === 'history');
      else if (r.view !== 'new') { this.note = null; this.noteState = 'idle'; }
    }
    if (r.name === 'plugins') void this.loadPlugins(r.space);
  }

  resetSaveState() { this.saveError = null; this.conflict = null; this.#pendingSave = null; }

  // ---- notes ----
  async loadNotes(space: string, force = false) {
    if (!force && this.notesSpace === space && this.notesState !== 'idle' && this.notesState !== 'error') return;
    if (this.notesSpace !== space) this.notes = [];
    this.notesSpace = space; this.notesState = 'loading'; this.notesError = null;
    const ep = this.#epoch;
    try {
      const d = await api.invoke<{ notes: NoteSummary[] }>('notes.list', { space_id: space });
      if (ep !== this.#epoch || this.notesSpace !== space) return;
      this.notes = d.notes; this.notesState = 'ready';
    } catch (e) {
      this.handleAuth(e, ep);
      if (ep !== this.#epoch || this.notesSpace !== space) return;
      this.notesState = 'error'; this.notesError = describeError(e);
    }
  }

  async openNote(space: string, id: string, wantHistory = false) {
    if (this.note && this.note.id === id && this.note.space_id === space) {
      if (wantHistory) void this.loadHistory();
      return;
    }
    this.noteState = 'loading'; this.noteError = null; this.note = null; this.resetSaveState(); this.insights = null; this.insightsError = null;
    this.history = []; this.historyState = 'idle';
    const ep = this.#epoch;
    try {
      const d = await api.invoke<{ note: Note }>('notes.get', { space_id: space, id });
      const r = this.route;
      if (ep !== this.#epoch || r.name !== 'notes' || r.id !== id || r.space !== space) return; // navigated away / session changed
      const kept = this.drafts[this.draftKey(space, id)];
      if (kept?.base) {
        // Restore the retained draft on top of its ORIGINAL base revision: if the server moved on, saving raises a conflict instead of overwriting.
        this.note = kept.base; this.draftTitle = kept.title; this.draftMarkdown = kept.markdown;
        if (kept.base.revision !== d.note.revision) { this.conflict = d.note; this.log('warning', `“${d.note.title}” changed on the server while your draft was kept`, hrefFor({ name: 'notes', space, id })); }
      } else { this.note = d.note; this.draftTitle = d.note.title; this.draftMarkdown = d.note.markdown; }
      this.noteState = 'ready';
      if (wantHistory) void this.loadHistory();
    } catch (e) {
      this.handleAuth(e, ep);
      const r = this.route;
      if (ep !== this.#epoch || r.name !== 'notes' || r.id !== id || r.space !== space) return;
      this.noteState = 'error';
      this.noteError = { code: e instanceof ApiError ? e.code : 'internal', message: describeError(e) };
    }
  }

  async createNote(): Promise<void> {
    const space = this.currentSpace; if (!space || this.saving) return;
    const title = this.draftTitle.trim();
    if (!title) { this.saveError = 'Give the note a title first.'; return; }
    await this.runSave('notes.create', { space_id: space, title, markdown: this.draftMarkdown }, (n) => {
      const r = this.route;
      if (r.name === 'notes' && r.view === 'new' && r.space === space) this.go({ name: 'notes', space, id: n.id }, true);
      void this.loadNotes(space, true);
    });
  }

  async saveNote(): Promise<void> {
    if (!this.note || this.saving || !this.dirty) return;
    const n = this.note;
    const title = this.draftTitle.trim();
    if (!title) { this.saveError = 'Title cannot be empty.'; return; }
    await this.runSave('notes.update',
      { space_id: n.space_id, id: n.id, base_revision: n.revision, title, markdown: this.draftMarkdown },
      (saved, stillOpen) => {
        if (stillOpen) { this.note = saved; this.draftTitle = saved.title; this.draftMarkdown = saved.markdown; this.insights = null; this.historyState = 'idle'; }
        const i = this.notes.findIndex((x) => x.id === saved.id);
        if (i >= 0) { const { markdown: _m, ...sum } = saved; this.notes[i] = sum; }
        void this.loadNotes(saved.space_id, true);
        if (stillOpen && this.route.name === 'notes' && this.route.view === 'history') void this.loadHistory();
      });
  }

  private async runSave(op: string, input: Record<string, unknown>, ok: (n: Note, stillOpen: boolean) => void) {
    this.saving = true; this.saveError = null; this.conflict = null;
    const ep = this.#epoch;
    const space0 = String(input.space_id), id0 = String(input.id ?? 'new');
    // Stable request id for an identical retry (e.g. after a network failure); new id once input changes.
    const key = op + JSON.stringify(input);
    if (!this.#pendingSave || this.#pendingSave.key !== key) this.#pendingSave = { key, requestId: api.newRequestId() };
    try {
      const d = await api.invoke<{ note: Note }>(op, input, this.#pendingSave.requestId);
      if (ep !== this.#epoch) return; // session changed: drop the result, touch nothing
      this.#pendingSave = null;
      delete this.drafts[this.draftKey(space0, id0)]; delete this.drafts[this.draftKey(space0, 'new')];
      ok(d.note, this.note?.id === d.note.id);
      this.log('success', `Saved “${d.note.title}”`, hrefFor({ name: 'notes', space: d.note.space_id, id: d.note.id }));
    } catch (e) {
      this.handleAuth(e, ep);
      if (ep !== this.#epoch) return;
      if (e instanceof ApiError && e.isConflict && this.note && this.note.id === id0) {
        this.#pendingSave = null;
        this.saveError = null;
        try { this.conflict = (await api.invoke<{ note: Note }>('notes.get', { space_id: this.note.space_id, id: this.note.id })).note; }
        catch { this.conflict = null; this.saveError = 'Conflict: a newer revision exists. Your draft is kept.'; }
        this.log('warning', `Conflict saving “${this.note.title}”; draft kept`, hrefFor({ name: 'notes', space: this.note.space_id, id: this.note.id }));
      } else {
        // keep #pendingSave: retrying the same input reuses the request id (server dedupes)
        this.saveError = describeError(e) + (e instanceof ApiError && e.code === 'unavailable' ? ' Your draft is kept; retry is safe.' : ' Your draft is kept.');
        this.log('error', `Save failed: ${describeError(e)}`);
      }
    } finally { if (ep === this.#epoch) this.saving = false; }
  }

  /** After a conflict: adopt the latest server revision as the base but keep the user's draft text. */
  keepMineOverLatest() {
    if (!this.conflict || !this.note) return;
    this.note = { ...this.note, revision: this.conflict.revision, updated_at: this.conflict.updated_at, title: this.conflict.title, markdown: this.conflict.markdown };
    this.conflict = null; // draft fields untouched; Save now targets the latest revision
  }
  discardDraftForLatest() {
    if (!this.conflict) return;
    if (this.note) delete this.drafts[this.draftKey(this.note.space_id, this.note.id)];
    const latest = this.conflict;
    this.note = latest; this.draftTitle = latest.title; this.draftMarkdown = latest.markdown; this.conflict = null;
    void this.loadNotes(latest.space_id, true);
  }
  revertDraft() { if (this.note) { delete this.drafts[this.draftKey(this.note.space_id, this.note.id)]; this.draftTitle = this.note.title; this.draftMarkdown = this.note.markdown; this.saveError = null; } }
  restoreVersion(v: NoteVersion) { this.draftTitle = v.title; this.draftMarkdown = v.markdown; }

  async loadHistory(more = false) {
    const n = this.note; if (!n) return;
    this.historyState = 'loading'; this.historyError = null;
    const ep = this.#epoch;
    try {
      const input: Record<string, unknown> = { space_id: n.space_id, id: n.id, limit: 20 };
      if (more && this.historyNext) input.cursor = this.historyNext;
      const d = await api.invoke<{ versions: NoteVersion[]; next_cursor: string | null }>('notes.history', input);
      if (ep !== this.#epoch || this.note?.id !== n.id) return;
      this.history = more ? [...this.history, ...d.versions] : d.versions;
      this.historyNext = d.next_cursor; this.historyState = 'ready';
    } catch (e) { this.handleAuth(e, ep); if (ep !== this.#epoch || this.note?.id !== n.id) return; this.historyState = 'error'; this.historyError = describeError(e); }
  }

  // ---- plugins ----
  async loadPlugins(space: string, force = false) {
    if (!force && this.pluginsSpace === space && this.pluginsState !== 'idle' && this.pluginsState !== 'error') return;
    if (this.pluginsSpace !== space) this.plugins = [];
    this.pluginsSpace = space; this.pluginsState = 'loading'; this.pluginsError = null;
    const ep = this.#epoch;
    try {
      const d = await api.invoke<{ plugins: PluginEntry[] }>('plugins.list', { space_id: space });
      if (ep !== this.#epoch || this.pluginsSpace !== space) return;
      this.plugins = d.plugins; this.pluginsState = 'ready';
    } catch (e) { this.handleAuth(e, ep); if (ep !== this.#epoch || this.pluginsSpace !== space) return; this.pluginsState = 'error'; this.pluginsError = describeError(e); }
  }

  async setPluginEnabled(space: string, id: string, enable: boolean) {
    this.pluginBusy = id;
    const ep = this.#epoch;
    try {
      const d = await api.invoke<{ plugin: PluginEntry }>(enable ? 'plugins.enable' : 'plugins.disable', { space_id: space, plugin_id: id });
      if (ep !== this.#epoch) return null;
      if (this.pluginsSpace === space) this.plugins = this.plugins.map((p) => (p.manifest.id === id ? d.plugin : p));
      this.insights = null;
      this.log('success', `${enable ? 'Enabled' : 'Disabled'} plugin ${d.plugin.manifest.name}`, hrefFor({ name: 'plugins', space, plugin: id }));
      return null;
    } catch (e) {
      this.handleAuth(e, ep);
      if (ep !== this.#epoch) return null;
      this.log('error', `Could not ${enable ? 'enable' : 'disable'} ${id}: ${describeError(e)}`);
      return describeError(e);
    } finally { if (ep === this.#epoch) this.pluginBusy = null; }
  }

  async previewPlugin(space: string, id: string): Promise<GenUiView> {
    const d = await api.invoke<{ view: GenUiView; preview: boolean }>('plugins.preview', { space_id: space, plugin_id: id });
    return d.view;
  }

  /** Extension plugin that contributes to notes.detail and is currently enabled. */
  get insightsPlugin(): PluginEntry | null {
    return this.pluginsSpace === this.currentSpace
      ? this.plugins.find((p) => p.enabled && p.manifest.operations.some((o) => o.id === 'notes-insights.summarize')) ?? null
      : null;
  }

  async summarize() {
    const n = this.note; if (!n) return;
    this.insightsBusy = true; this.insightsError = null;
    const ep = this.#epoch;
    try {
      const result = await api.invoke<SummarizeResult>('notes-insights.summarize', { space_id: n.space_id, id: n.id });
      if (ep === this.#epoch && this.note?.id === n.id && this.note.revision === n.revision) this.insights = { noteId: n.id, revision: n.revision, result };
    } catch (e) { this.handleAuth(e, ep); if (ep === this.#epoch && this.note?.id === n.id) this.insightsError = describeError(e); }
    finally { if (ep === this.#epoch) this.insightsBusy = false; }
  }

  /** Generic dispatch used by GenUI buttons (host re-validates and re-authorizes). */
  async dispatch(operation: string, input: Record<string, unknown>) {
    return api.invoke<any>(operation, input);
  }
}

export const app = new AppStore();
