/** Worker for the Calternal Mail data operations. IMAPS and secrets stay in the host. */
import { listItems, prepareContent, serveWorker } from "@calternal/plugin-sdk";
import type { WorkerContextV2, WorkerHandlerV2 } from "@calternal/plugin-sdk";
import manifest from "../manifest.json" with { type: "json" };

type Endpoint = { host: string; port: number; username: string };
type ItemRef = { id: string; revision: string };
type AccountInput = { account_id: string };
type FolderInput = AccountInput & { folder: string };
type Cursor = { before: number; uid_validity: number };
type MessageInput = FolderInput & { uid: number; uid_validity: number };
type MailResult = {
  header: { subject: string | null };
  text: string | null;
  attachments: unknown[];
  has_html: boolean;
  snapshot_key: string;
  eml: string;
};
const ACCOUNTS = "mail-account";
const MESSAGES = "mail-message";

async function account(context: WorkerContextV2, id: string): Promise<Endpoint> {
  const item = await context.call("read_item", { kind: ACCOUNTS, item_id: id }) as { metadata: Endpoint };
  const { host, port, username } = item.metadata;
  return { host, port, username };
}

async function mail(context: WorkerContextV2, endpoint: Endpoint, action: object) {
  return context.call("mail_v1", { connection: "inbox", endpoint, action });
}

async function frame(context: WorkerContextV2, item: ItemRef, hasHtml: boolean) {
  if (!hasHtml) return null;
  return (await prepareContent(context, {
    kind: MESSAGES, item_id: item.id, revision: item.revision, field: "mail_html",
  })).frame_id;
}

const handlers = {
  "calternal-mail.add-account": async (value, context) => {
    const { host, port, username } = value as Endpoint;
    const endpoint = { host: host.toLowerCase(), port, username };
    const result = await mail(context, endpoint, { type: "request_password" }) as
      { ready: boolean; secret_request_id?: string; endpoint_key: string };
    const { endpoint_key, ...prompt } = result;
    const existing = (await listItems(context, { kind: ACCOUNTS, label: endpoint_key, limit: 1 })).items[0];
    const saved = existing ?? await context.call("write_item", {
      kind: ACCOUNTS, metadata: { title: endpoint.host, labels: [endpoint_key], ...endpoint },
      text: `IMAPS account on ${endpoint.host}:${endpoint.port}.\n`,
    }) as ItemRef;
    return { id: saved.id, revision: saved.revision, ...prompt };
  },
  "calternal-mail.list-accounts": async (value, context) => {
    const { cursor } = value as { cursor?: string };
    const page = await listItems(context, { kind: ACCOUNTS, cursor, limit: 16 });
    return { accounts: page.items, next_cursor: page.next_cursor };
  },
  "calternal-mail.list-folders": async (value, context) => {
    const { account_id } = value as AccountInput;
    return mail(context, await account(context, account_id), { type: "folders" });
  },
  "calternal-mail.list-headers": async (value, context) => {
    const { account_id, folder, cursor } = value as FolderInput & { cursor?: Cursor };
    const result = await mail(context, await account(context, account_id), {
      type: "headers", folder, ...cursor,
    }) as { headers: unknown[]; uid_validity: number; next_before: number | null };
    return {
      headers: result.headers, uid_validity: result.uid_validity,
      next_cursor: result.next_before === null ? null : {
        before: result.next_before, uid_validity: result.uid_validity,
      },
    };
  },
  "calternal-mail.read-message": async (value, context) => {
    const { account_id, ...message } = value as MessageInput;
    const result = await mail(context, await account(context, account_id), {
      type: "read", ...message,
    }) as MailResult;
    const existing = (await listItems(context, {
      kind: MESSAGES, label: result.snapshot_key, limit: 1,
    })).items[0];
    // An IMAP UID identifies immutable content until UIDVALIDITY changes.
    const saved = existing ?? await context.call("write_item", {
      kind: MESSAGES, text: result.eml,
      metadata: {
        title: result.header.subject || "Mail message", labels: [result.snapshot_key],
        has_html: result.has_html,
      },
    }) as ItemRef;
    return {
      id: saved.id, revision: saved.revision, header: result.header, text: result.text,
      attachments: result.attachments, frame_id: await frame(context, saved, result.has_html),
    };
  },
  "calternal-mail.mark-read": async (value, context) => {
    const { account_id, ...message } = value as MessageInput;
    return mail(context, await account(context, account_id), { type: "mark_read", ...message });
  },
  "calternal-mail.open-snapshot": async (value, context) => {
    const { item_id } = value as { item_id: string };
    const item = await context.call("read_item", { kind: MESSAGES, item_id }) as
      ItemRef & { metadata: { has_html: boolean } };
    return { id: item.id, revision: item.revision, frame_id: await frame(context, item, item.metadata.has_html) };
  },
} satisfies Record<string, WorkerHandlerV2>;

serveWorker(manifest, handlers, process.stdin, line =>
  new Promise<void>((resolve, reject) =>
    process.stdout.write(line, error => error ? reject(error) : resolve())),
).catch(() => {
  process.stderr.write("calternal-mail worker protocol failure\n");
  process.exitCode = 1;
  process.stdin.destroy();
});
