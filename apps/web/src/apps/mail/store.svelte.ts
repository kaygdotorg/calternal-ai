/**
 * Reactive state for the Mail app: accounts, the open account's folders, one folder's headers (paged) and the open message. Screens are GenUI descriptions built from this state
 * (plugins/calternal-mail/src/views.ts); the commands the descriptions set (`$go`, `$folder`, `$account`) come back through `apply`. Marking read happens when a message is opened,
 * after it has been shown, and never blocks showing it. Nothing here keeps a password: the account form has no such field.
 */
import { listView, messageView, setupView, type SetupDraft } from "@calternal/mail/views";
import { FixtureMailClient } from "./fixtures";
import type { HeaderCursor, MailAccountRef, MailClient, MailFolder, MailHeader, MailMessage } from "./client";

const message = (e: unknown, fallback: string) => (e instanceof Error && e.message ? e.message : fallback);

/** What the screens did that the shell must do (navigation), reported to the component that owns the router. */
export interface MailEffects { open(uid: number): void; setup(): void; leave(): void }

/** Mail's state as runes. */
export class MailStore {
  client: MailClient;
  accounts = $state<MailAccountRef[]>([]);
  account = $state<string | null>(null);
  folders = $state<MailFolder[]>([]);
  folder = $state<string | null>(null);
  headers = $state<MailHeader[]>([]);
  cursor = $state<HeaderCursor | null>(null);
  validity = $state(0);
  selected = $state<number | null>(null);
  open = $state<MailMessage | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  /** Why the open account cannot list: it waits for its password. */
  notice = $state<string | null>(null);
  ready = $state(false);
  setupDraft = $state<SetupDraft>({ host: "", port: "993", username: "" });
  setupError = $state<string | null>(null);
  setupWaiting = $state(false);
  messageError = $state<string | null>(null);
  #seq = 0;

  constructor(client: MailClient = new FixtureMailClient()) { this.client = client; }

  /** Loads the accounts, and the first account's inbox. */
  async start(): Promise<void> {
    if (this.ready) return;
    try {
      this.accounts = await this.client.accounts();
      this.ready = true;
      if (this.accounts[0]) await this.useAccount(this.accounts[0].id);
    } catch (e) { this.error = message(e, "Mail could not load. Try again."); this.ready = true; }
  }

  /** Reloads the open folder from its first page (pull to refresh): the header feed is a live view, so a refresh starts again. */
  async refresh(): Promise<void> { if (this.account && this.folder) await this.useFolder(this.folder); else if (this.account) await this.useAccount(this.account); }

  async useAccount(id: string): Promise<void> {
    const seq = ++this.#seq;
    this.account = id; this.folders = []; this.folder = null; this.headers = []; this.cursor = null; this.error = null; this.notice = null; this.selected = null; this.open = null;
    this.loading = true;
    try {
      const folders = await this.client.folders(id);
      if (seq !== this.#seq) return;
      this.folders = folders;
      const first = folders.find((f) => f.selectable && f.name === "INBOX") ?? folders.find((f) => f.selectable);
      if (first) await this.useFolder(first.name); else this.loading = false;
    } catch (e) {
      if (seq !== this.#seq) return;
      this.notice = message(e, "This account could not be listed."); this.loading = false;
    }
  }

  async useFolder(name: string): Promise<void> {
    const seq = ++this.#seq, account = this.account;
    if (!account) return;
    this.folder = name; this.headers = []; this.cursor = null; this.error = null; this.loading = true;
    try {
      const page = await this.client.headers(account, name);
      if (seq !== this.#seq) return;
      this.headers = page.headers; this.cursor = page.next_cursor; this.validity = page.uid_validity;
    } catch (e) { if (seq === this.#seq) this.error = message(e, "This folder could not be listed. Try again."); }
    if (seq === this.#seq) this.loading = false;
  }

  /** The next page of the open folder, appended; a failure keeps what is shown and says so. */
  async more(): Promise<void> {
    const { account, folder, cursor } = this;
    if (!account || !folder || !cursor || this.loading) return;
    this.loading = true;
    try {
      const page = await this.client.headers(account, folder, cursor);
      if (account !== this.account || folder !== this.folder) return;
      const have = new Set(this.headers.map((h) => h.uid));
      this.headers = [...this.headers, ...page.headers.filter((h) => !have.has(h.uid))];
      this.cursor = page.next_cursor; this.validity = page.uid_validity;
    } catch (e) { this.error = message(e, "More messages could not be loaded."); }
    this.loading = false;
  }

  /** Opens a message in the pane, then marks it read (the list's mark follows when the server confirms). */
  async view(uid: number): Promise<void> {
    const { account, folder, validity } = this;
    this.selected = uid; this.messageError = null;
    if (!account || !folder) { this.open = null; return; }
    const seq = ++this.#seq;
    this.open = null;
    try {
      const m = await this.client.read(account, folder, validity, uid);
      if (seq !== this.#seq) return;
      this.open = m;
      if (!m.header.read) {
        void this.client.markRead(account, folder, validity, uid).then(() => {
          this.headers = this.headers.map((h) => (h.uid === uid ? { ...h, read: true } : h));
          if (this.open?.header.uid === uid) this.open = { ...this.open, header: { ...this.open.header, read: true } };
        }, () => { this.messageError = "The message is shown, but it could not be marked as read."; });
      }
    } catch (e) { if (seq === this.#seq) this.messageError = message(e, "This message could not be opened."); }
  }

  /** Adds an account from the form's three fields; the password is asked for separately by the host's protected prompt. */
  async add(): Promise<boolean> {
    const d = this.setupDraft, port = Number(d.port);
    this.setupError = null; this.setupWaiting = false;
    if (!d.host.trim() || !d.username.trim()) { this.setupError = "Enter the server name and your username."; return false; }
    if (!Number.isInteger(port) || port < 1 || port > 65535) { this.setupError = "The port is a number from 1 to 65535, usually 993."; return false; }
    try {
      const added = await this.client.addAccount({ host: d.host.trim(), port, username: d.username.trim() });
      this.accounts = await this.client.accounts();
      this.setupWaiting = added.waiting;
      if (!added.waiting) await this.useAccount(added.id);
      return true;
    } catch (e) { this.setupError = message(e, "The account could not be added."); return false; }
  }

  /** The sidebar's GenUI source. */
  get listSource(): string {
    return listView({ accounts: this.accounts, account: this.account, notice: this.notice, folders: this.folders, folder: this.folder, headers: this.headers, more: !!this.cursor, selected: this.selected, error: this.error, loading: this.loading });
  }
  /** The setup screen's source. */
  setupSource(back: boolean): string { return setupView(this.setupDraft, { error: this.setupError, waiting: this.setupWaiting, back }); }
  /** The reading pane's source, or null while there is no message to draw. */
  get messageSource(): string | null { return this.open ? messageView(this.open) : null; }

  /**
   * Performs what a screen's state asks for. `values` is the description's state after a change; a command in `go` wins, then a changed selector. Returns nothing: the screens redraw from the store.
   */
  apply(values: Record<string, unknown>, fx: MailEffects): void {
    const go = typeof values.go === "string" ? values.go : "";
    if (go.startsWith("open:")) { const uid = Number(go.slice(5)); if (Number.isInteger(uid)) fx.open(uid); }
    else if (go === "more") void this.more();
    else if (go === "setup") fx.setup();
    else if (go === "back") fx.leave();
    else if (go === "add") {
      this.setupDraft = { host: String(values.host ?? ""), port: String(values.port ?? ""), username: String(values.username ?? "") };
      void this.add().then((ok) => { if (ok && !this.setupWaiting) fx.leave(); });
    } else {
      if (typeof values.account === "string" && values.account && values.account !== this.account) void this.useAccount(values.account);
      else if (typeof values.folder === "string" && values.folder && values.folder !== this.folder) void this.useFolder(values.folder);
    }
  }
}

/** The app's one store. */
export const mail = new MailStore();
