/**
 * Mail's screens as portable GenUI source (packages/genui). Pure functions from the shapes the Mail operations return to a
 * description the shell draws with its own components; nothing here performs I/O, holds a password or reaches a mailbox.
 *
 * How a screen talks back: GenUI has no action that invokes an operation, so every screen declares a `$go` state variable and its
 * buttons `Set` it to a command (`open:<uid>`, `more`, `add`, `setup`, `back`). The client that draws the description reads the
 * state after each change, performs the matching operation and draws the next screen. `$account` and `$folder` are bound to the
 * selectors the same way. `$host`, `$port` and `$username` are the account form's fields; there is deliberately no field for a
 * password, which only the host's protected prompt ever receives.
 */

/** One folder as `calternal-mail.list-folders` returns it. `name` is the IMAP wire form (modified UTF-7) and is what later requests send. */
export interface MailFolder { name: string; delimiter: string | null; selectable: boolean }
/** One header as `calternal-mail.list-headers` returns it. `from` holds bare addresses; the host does not return display names. */
export interface MailHeader { uid: number; subject: string | null; from: string[]; date: string | null; read: boolean; bytes: number | null }
/** One attachment's metadata; its content is never requested. */
export interface MailAttachment { filename: string | null; media_type: string; bytes: number | null }
/** The part of `calternal-mail.read-message`'s result a screen draws. */
export interface MailMessage { header: MailHeader; text: string | null; attachments: MailAttachment[]; has_html: boolean; frame_id?: string | null }
/** An account summary for the selector. */
export interface MailAccountRef { id: string; title: string; username: string }

/** The list screen (the sidebar): accounts, folders and one page of headers. */
export interface ListScreen {
  accounts: MailAccountRef[];
  account: string | null;
  /** Why the account cannot list yet (for instance it still waits for its password); drawn instead of folders. */
  notice?: string | null;
  folders: MailFolder[];
  folder: string | null;
  headers: MailHeader[];
  /** True when `list-headers` returned a continuation. */
  more: boolean;
  selected: number | null;
  /** A plain-language failure from the last operation. */
  error?: string | null;
  loading?: boolean;
}

/** A GenUI string literal for `text`: every character that could end the literal or start a template is escaped, so mail content can never become code. */
export function lit(text: string): string {
  let out = '"';
  for (const ch of text.replace(/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/g, "\ufffd")) {
    const c = ch.codePointAt(0)!;
    if (ch === '"' || ch === "\\" || ch === "$") out += "\\" + ch;
    else if (ch === "\n") out += "\\n";
    else if (ch === "\t") out += "\\t";
    else if (c < 0x20 || c === 0x7f || c === 0x2028 || c === 0x2029) out += `\\u{${c.toString(16)}}`;
    else out += ch;
  }
  return out + '"';
}

/** Decodes an IMAP folder name (modified UTF-7, RFC 3501 5.1.3) for display; a name that does not decode is shown as the server sent it. */
export function folderLabel(wire: string): string {
  try {
    return wire.replace(/&([^-]*)-/g, (_, b64: string) => {
      if (!b64) return "&";
      const bin = atob(b64.replace(/,/g, "/") + "=".repeat((4 - (b64.length % 4)) % 4));
      let s = "";
      for (let i = 0; i + 1 < bin.length; i += 2) s += String.fromCharCode((bin.charCodeAt(i) << 8) | bin.charCodeAt(i + 1));
      return s;
    });
  } catch {
    return wire;
  }
}

const KB = 1024;
/** A size in plain units ("82 KB"); an attachment's size is the encoded size, so it reads as about that. */
export function sizeText(bytes: number | null): string {
  if (bytes === null || bytes < 0) return "";
  if (bytes < KB) return `${bytes} B`;
  if (bytes < KB * KB) return `${Math.round(bytes / KB)} KB`;
  return `${(bytes / (KB * KB)).toFixed(1)} MB`;
}

/** The date as a short local time when the header parses, else the header's own text. */
export function dateText(date: string | null, now = Date.now()): string {
  if (!date) return "";
  const t = Date.parse(date);
  if (Number.isNaN(t)) return date;
  const d = new Date(t);
  const sameDay = new Date(now).toDateString() === d.toDateString();
  return sameDay ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : d.toLocaleDateString([], { month: "short", day: "numeric", year: d.getFullYear() === new Date(now).getFullYear() ? undefined : "numeric" });
}

/** Text drawn through a Text node is Markdown, so mail-controlled words there lose the characters that could make a link or an image (an image would load from the network). */
const plain = (text: string): string => text.replace(/[\[\]()<>!*_`~\\]/g, "");

const options = (list: { value: string; label: string }[]) => "[" + list.map((o) => `{value: ${lit(o.value)}, label: ${lit(o.label)}}`).join(", ") + "]";
const SHORT_LIST = 5;

/** The list screen as GenUI source. A selector for accounts (only when there are several), one for folders, the headers, then "Older messages" and "Add an account". */
export function listView(s: ListScreen, now = Date.now()): string {
  const out: string[] = [`$go=""`];
  const parts: string[] = [];
  if (s.accounts.length > 1) {
    out.push(`$account=${lit(s.account ?? "")}`);
    parts.push(`Select($account, ${options(s.accounts.map((a) => ({ value: a.id, label: a.title })))}, "Account")`);
  }
  const folders = s.folders.filter((f) => f.selectable);
  if (folders.length) {
    out.push(`$folder=${lit(s.folder ?? "")}`);
    const opts = options(folders.map((f) => ({ value: f.name, label: folderLabel(f.name) })));
    parts.push(folders.length <= SHORT_LIST ? `Choice($folder, ${opts}, "Folder")` : `Select($folder, ${opts}, "Folder")`);
  }
  if (s.error) parts.push(`Callout(${lit(s.error)}, "bad", "Something went wrong")`);
  if (s.notice) parts.push(`Callout(${lit(s.notice)}, "info")`);
  for (const h of s.headers) {
    const subject = h.subject?.trim() || "(no subject)";
    const sender = h.from[0] ?? "Unknown sender";
    const date = dateText(h.date, now);
    const head = [`Button(${lit(subject)}, Set($go, ${lit("open:" + h.uid)}), "secondary")`];
    if (s.selected === h.uid) head.push(`Badge("Open", "neutral")`);
    else if (!h.read) head.push(`Badge("Unread", "info")`);
    parts.push(`Stack([Row([${head.join(", ")}]), Text(${lit(plain([sender, date].filter(Boolean).join(" · ")))})])`);
  }
  if (s.loading) parts.push(`Text("Loading…")`);
  else if (s.folder && !s.headers.length && !s.error) parts.push(`Text("No messages in this folder.")`);
  if (s.more) parts.push(`Button("Older messages", Set($go, "more"))`);
  parts.push(`Button("Add an account", Set($go, "setup"))`);
  return out.join("\n") + `\nStack([${parts.join(", ")}])`;
}

/** What the account form was filled with, so a failed attempt keeps the person's typing. */
export interface SetupDraft { host: string; port: string; username: string }

/** The account form: server, port and username, and a statement that the password is asked for separately. `error` and `waiting` say what the last attempt did. */
export function setupView(draft: SetupDraft, state: { error?: string | null; waiting?: boolean; back?: boolean } = {}): string {
  const body: string[] = [];
  if (state.error) body.push(`Callout(${lit(state.error)}, "bad", "Could not add the account")`);
  if (state.waiting) body.push(`Callout("The account is saved. Calternal asks for its password in a protected prompt that no app or agent can read; the account lists mail once the prompt is answered.", "info", "Waiting for the password")`);
  body.push(`Form([Input($host, "Mail server", "imap.example.com"), Input($port, "Port", "993"), Input($username, "Username", "you@example.com"), Button("Add account", Set($go, "add"), "primary")], "Account details")`);
  body.push(`Text("Mail connects over IMAPS with a password. The password is never typed into this form.")`);
  if (state.back) body.push(`Button("Back to mail", Set($go, "back"))`);
  return `$go=""\n$host=${lit(draft.host)}\n$port=${lit(draft.port)}\n$username=${lit(draft.username)}\nStack([${body.join(", ")}])`;
}

/** The reading pane's body (the subject is the page's heading, drawn by the app around this). Plain text, attachment metadata and what is not shown yet. */
export function messageView(m: MailMessage, now = Date.now()): string {
  const h = m.header;
  const pairs = [`"From": ${lit(h.from.join(", ") || "Unknown sender")}`, `"Date": ${lit(dateText(h.date, now) || "Unknown")}`];
  if (h.bytes !== null) pairs.push(`"Size": ${lit(sizeText(h.bytes))}`);
  const parts = [`KeyValue({${pairs.join(", ")}})`];
  if (m.has_html) {
    parts.push(`Callout("This message has an HTML version; this view shows its plain text. Remote images load only on request, from the sealed frame, which this view cannot draw yet.", "info")`);
  }
  parts.push(m.text ? `Code(${lit(m.text)})` : `Callout("This message has no plain-text part.", "neutral")`);
  if (m.attachments.length) {
    const rows = m.attachments.map((a) => `[${lit(a.filename ?? "(unnamed)")}, ${lit(a.media_type)}, ${lit(sizeText(a.bytes))}]`);
    parts.push(`Details(${lit(`Attachments (${m.attachments.length})`)}, [Table(["Name", "Type", "Size"], [${rows.join(", ")}]), Text("Attachments are listed only. Mail cannot download them yet.")], true)`);
  }
  return `$go=""\nStack([${parts.join(", ")}])`;
}

/** The pane's empty state, before a message is chosen. */
export const EMPTY_PANE = `Text("Choose a message from the list.")`;
