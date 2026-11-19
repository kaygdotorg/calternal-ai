/**
 * Fixture mailbox and `FixtureMailClient`: fabricated, deterministic, in memory. It never contacts a server and holds no password; the real data layer's own tests use a fixture IMAP server
 * (crates/server/src/plugins/broker/mail), and this is the web app's counterpart for the screens. Fixture-only; none of it ships with the real client.
 */
import type { AddedAccount, HeaderCursor, HeaderPage, MailAccountRef, MailClient, MailFolder, MailHeader, MailMessage } from "./client";

const BASE = Date.UTC(2026, 10, 18, 9, 0, 0);
const HOUR = 3_600_000;
const VALIDITY = 7;
const PAGE = 20;

/** The account every fixture session starts with. */
export const FIXTURE_ACCOUNT: MailAccountRef = { id: "00000000-0000-4000-8000-00000000a001", title: "mail.example.test", username: "you@example.test" };

const FOLDERS: MailFolder[] = [
  { name: "INBOX", delimiter: "/", selectable: true },
  { name: "Drafts", delimiter: "/", selectable: true },
  { name: "Sent", delimiter: "/", selectable: true },
  { name: "Archive", delimiter: "/", selectable: true },
  { name: "Archive/&AOk-t&AOk-", delimiter: "/", selectable: true },
  { name: "Junk", delimiter: "/", selectable: true },
  { name: "[Gmail]", delimiter: "/", selectable: false },
];

interface Seed { from: string; subject: string | null; text: string | null; html?: boolean; attach?: [string, string, number][]; unread?: boolean }

const LONG = "A very long unbroken line https://example.test/a/really/long/path/that/keeps/going/and/going/and/going/without/any/spaces/so/the/pane/must/scroll/or/wrap/instead/of/breaking/the/layout?token=" + "x".repeat(120);
const SPECIAL: Seed[] = [
  { from: "ada@example.test", subject: "Welcome to the fixture mailbox", text: "Hello,\n\nThis is a plain-text message with a few short lines.\n\n  An indented line, kept as written.\n\nBest,\nAda\n", unread: true },
  { from: "billing@shop.example.test", subject: "Your receipt for order #4411", text: "Thanks for your order.\n\nItem            Qty   Price\nNotebook          2   $18.00\n\nTotal: $18.00\n", html: true, attach: [["receipt-4411.pdf", "application/pdf", 83_200]], unread: true },
  { from: "news@letters.example.test", subject: "This week: images, tables and a $5 sale ${not code}", text: "Plain alternative of a newsletter.\n\n[link](https://example.test) and ![tracker](https://tracker.example.test/pixel.png) stay literal text here.\n", html: true },
  { from: "team@work.example.test", subject: "Quarterly plan — draft 3", text: "Attached are the plan and the budget.\nPlease reply by Friday.", attach: [["plan.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document", 412_000], ["budget.xlsx", "application/vnd.ms-excel", 1_950_000], ["", "image/png", 2_100]] },
  { from: "bot@alerts.example.test", subject: null, text: "A message with no subject line.\n" },
  { from: "html-only@example.test", subject: "HTML only, no plain text", text: null, html: true },
  { from: "long@example.test", subject: 'Quotes "and" backslashes \\ in a very long subject that goes on and on until it has to wrap onto a second line on a phone', text: LONG + "\n" + "word ".repeat(80) },
];

function build(i: number): { header: MailHeader; seed: Seed } {
  const uid = 200 - i;
  const seed = i < SPECIAL.length ? SPECIAL[i]! : { from: `person${i}@example.test`, subject: `Fixture message ${uid}`, text: `Body of message ${uid}.\n`, unread: i % 7 === 0 };
  return { header: { uid, subject: seed.subject, from: [seed.from], date: new Date(BASE - i * HOUR * 5).toUTCString(), read: !seed.unread, bytes: 1500 + i * 37 }, seed };
}

const wait = () => new Promise<void>((r) => setTimeout(r, 40));

/** In-memory `MailClient`. Opening a message does not change its flag; `markRead` does, as the plugin's operations do. */
export class FixtureMailClient implements MailClient {
  #accounts: MailAccountRef[] = [FIXTURE_ACCOUNT];
  #read = new Set<number>();

  async accounts(): Promise<MailAccountRef[]> { await wait(); return [...this.#accounts]; }

  async addAccount(e: { host: string; port: number; username: string }): Promise<AddedAccount> {
    await wait();
    if (/unreachable/i.test(e.host)) throw new Error("Could not reach that server. Check the name and the port, then try again.");
    const id = `00000000-0000-4000-8000-${String(this.#accounts.length + 1).padStart(12, "0")}`;
    this.#accounts.push({ id, title: e.host.toLowerCase(), username: e.username });
    return { id, waiting: true };
  }

  async folders(account: string): Promise<MailFolder[]> {
    await wait();
    if (account !== FIXTURE_ACCOUNT.id) throw new Error("This account has no password yet, so its folders cannot be listed.");
    return FOLDERS.map((f) => ({ ...f }));
  }

  async headers(account: string, folder: string, cursor?: HeaderCursor): Promise<HeaderPage> {
    await wait();
    if (account !== FIXTURE_ACCOUNT.id) throw new Error("This account has no password yet.");
    if (folder !== "INBOX") return { headers: [], uid_validity: VALIDITY, next_cursor: null };
    const from = cursor ? 200 - cursor.before + 1 : 0;
    const items = Array.from({ length: PAGE }, (_, k) => from + k).filter((i) => i < 45).map((i) => build(i).header).map((h) => ({ ...h, read: h.read || this.#read.has(h.uid) }));
    const last = items.at(-1);
    return { headers: items, uid_validity: VALIDITY, next_cursor: last && last.uid > 156 ? { before: last.uid, uid_validity: VALIDITY } : null };
  }

  async read(account: string, folder: string, uidValidity: number, uid: number): Promise<MailMessage> {
    await wait();
    if (uidValidity !== VALIDITY) throw new Error("This mailbox changed since the list was loaded. Refresh the folder.");
    const { header, seed } = build(200 - uid);
    if (folder !== "INBOX" || account !== FIXTURE_ACCOUNT.id || uid > 200 || uid < 156) throw new Error("That message is no longer in this folder.");
    return { header: { ...header, read: header.read || this.#read.has(uid) }, text: seed.text, has_html: !!seed.html, frame_id: seed.html ? "fixture-frame" : null, attachments: (seed.attach ?? []).map(([filename, media_type, bytes]) => ({ filename: filename || null, media_type, bytes })) };
  }

  async markRead(_account: string, _folder: string, _uidValidity: number, uid: number): Promise<void> { await wait(); this.#read.add(uid); }
}
