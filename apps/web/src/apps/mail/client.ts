/**
 * The Mail app's data surface: the same shapes the `calternal-mail.*` operations return (plugins/calternal-mail/README.md), so a server client can replace the fixture one
 * without the screens changing. The app draws only from these; the fixture client below is the one in use until the plugin's operations are reachable from the web client.
 */
import type { MailAccountRef, MailFolder, MailHeader, MailMessage } from "@calternal/mail/views";

export type { MailAccountRef, MailFolder, MailHeader, MailMessage };

/** Where the next page of headers starts: the plugin's own continuation, passed back unchanged with the same account and folder. */
export interface HeaderCursor { before: number; uid_validity: number }
/** One page of headers, most recent first. */
export interface HeaderPage { headers: MailHeader[]; uid_validity: number; next_cursor: HeaderCursor | null }
/** What adding an account returns: the saved account, and whether it still waits for its password in the protected prompt. */
export interface AddedAccount { id: string; waiting: boolean }

/** The operations the screens use. A failure is an `Error` whose message is plain language (the plugin's refusals carry no provider text). */
export interface MailClient {
  accounts(): Promise<MailAccountRef[]>;
  addAccount(endpoint: { host: string; port: number; username: string }): Promise<AddedAccount>;
  folders(account: string): Promise<MailFolder[]>;
  headers(account: string, folder: string, cursor?: HeaderCursor): Promise<HeaderPage>;
  /** Opens a message: its text, attachment list and whether it has HTML. Does not change flags. */
  read(account: string, folder: string, uidValidity: number, uid: number): Promise<MailMessage>;
  markRead(account: string, folder: string, uidValidity: number, uid: number): Promise<void>;
}
