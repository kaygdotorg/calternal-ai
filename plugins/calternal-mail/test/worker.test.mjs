import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
import { createInterface } from "node:readline";
import test from "node:test";

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url)));
const path = new URL("../dist/worker.js", import.meta.url);
const authority = {
  account_id: "01932910-0000-7000-8000-000000000001",
  space_id: "01932910-0000-7000-8000-000000000002",
  grant: "host-ref",
};
const endpoint = { host: "mail.example.test", port: 993, username: "reader" };
const account = { id: "01932910-0000-7000-8000-000000000003", revision: "account-revision" };
const snapshot = { id: "01932910-0000-7000-8000-000000000004", revision: "snapshot-revision" };

async function worker(run) {
  const child = spawn(process.execPath, [path.pathname], { stdio: ["pipe", "pipe", "pipe"] });
  const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  const done = once(child, "exit");
  let current = 0;
  try {
    await run(async (operation, input, callbacks) => {
      const invocation_id = `mail-${++current}`;
      const send = message => child.stdin.write(JSON.stringify({ protocol_version: 2, invocation_id, message }) + "\n");
      send({ type: "invoke", operation: `calternal-mail.${operation}`, input, authority });
      for (const [method, input, data] of callbacks) {
        const reply = JSON.parse((await lines.next()).value);
        assert.equal(reply.invocation_id, invocation_id);
        assert.deepEqual(reply.message, {
          type: "host_call", call_id: reply.message.call_id, method, input, authority,
        });
        send({ type: "host_reply", call_id: reply.message.call_id, ok: true, data });
      }
      const result = JSON.parse((await lines.next()).value);
      assert.equal(result.invocation_id, invocation_id);
      assert.equal(result.message.type, "complete");
      return result.message.data;
    });
    child.stdin.end();
    assert.equal((await done)[0], 0);
  } finally {
    child.kill();
  }
}

const callMail = (action, result) => ["mail_v1", { connection: "inbox", endpoint, action }, result];
const accountRead = ["read_item", { kind: "mail-account", item_id: account.id }, { metadata: endpoint }];

test("account add exposes only the protected prompt and stores endpoint details", { timeout: 5000 }, async () => {
  const op = manifest.operations.find(op => op.id === "calternal-mail.add-account");
  assert.deepEqual(op.input_schema.required, ["host", "port", "username"]);
  assert.equal(op.input_schema.additionalProperties, false);
  assert.equal(manifest.network.mail_connection, "inbox");
  await worker(async invoke => {
    const result = await invoke("add-account", endpoint, [
      callMail({ type: "request_password" }, { ready: false, secret_request_id: "prompt-id", endpoint_key: "account-label" }),
      ["list_items_v1", { kind: "mail-account", label: "account-label", limit: 1 }, { items: [] }],
      ["write_item", {
        kind: "mail-account",
        metadata: { title: endpoint.host, labels: ["account-label"], ...endpoint },
        text: "IMAPS account on mail.example.test:993.\n",
      }, account],
    ]);
    assert.deepEqual(result, { ...account, ready: false, secret_request_id: "prompt-id" });
  });
});

test("folder and header paging preserve remote identities and mark-read targets a UID", { timeout: 5000 }, async () => {
  const folder = { account_id: account.id, folder: "INBOX" };
  await worker(async invoke => {
    const folders = { folders: [{ name: "INBOX" }], truncated: false };
    assert.deepEqual(await invoke("list-folders", { account_id: account.id }, [
      accountRead, callMail({ type: "folders" }, folders),
    ]), folders);
    const headers = await invoke("list-headers", folder, [
      accountRead, callMail({ type: "headers", folder: "INBOX" }, { headers: [{ uid: 4 }], uid_validity: 2, next_before: 3 }),
    ]);
    assert.deepEqual(headers, { headers: [{ uid: 4 }], uid_validity: 2, next_cursor: { before: 3, uid_validity: 2 } });
    const last = await invoke("list-headers", { ...folder, cursor: headers.next_cursor }, [
      accountRead, callMail({ type: "headers", folder: "INBOX", before: 3, uid_validity: 2 },
        { headers: [{ uid: 1 }], uid_validity: 2, next_before: null }),
    ]);
    assert.equal(last.next_cursor, null);
    assert.deepEqual(await invoke("mark-read", { ...folder, uid: 1, uid_validity: 2 }, [
      accountRead, callMail({ type: "mark_read", folder: "INBOX", uid: 1, uid_validity: 2 }, { read: true }),
    ]), { read: true });
  });
});

test("opening a message stores EML and delivers only the sealed frame and attachment metadata", { timeout: 5000 }, async () => {
  const message = { folder: "INBOX", uid: 4, uid_validity: 2 };
  const header = { subject: "Test message" };
  const attachments = [{ filename: "fixture.pdf", bytes: 82 }];
  const eml = "Content-Type: text/html\r\n\r\n<script>payload</script>";
  const prepare = ["content_prepare_v1", {
    kind: "mail-message", item_id: snapshot.id, revision: snapshot.revision, field: "mail_html",
  }, { frame_id: "sealed" }];
  const calls = [
    accountRead, callMail({ type: "read", ...message }, {
      header, text: "only plain text", attachments, has_html: true, snapshot_key: "snapshot-label", eml,
    }),
  ];
  await worker(async invoke => {
    const input = { account_id: account.id, ...message };
    const data = await invoke("read-message", input, [
      ...calls,
      ["list_items_v1", { kind: "mail-message", label: "snapshot-label", limit: 1 }, { items: [] }],
      ["write_item", { kind: "mail-message", text: eml,
        metadata: { title: header.subject, labels: ["snapshot-label"], has_html: true } }, snapshot],
      prepare,
    ]);
    assert.deepEqual(data, { ...snapshot, header, text: "only plain text", attachments, frame_id: "sealed" });
    assert.deepEqual(await invoke("read-message", input, [
      ...calls,
      ["list_items_v1", { kind: "mail-message", label: "snapshot-label", limit: 1 }, { items: [snapshot] }],
      prepare,
    ]), data);
  });
});
