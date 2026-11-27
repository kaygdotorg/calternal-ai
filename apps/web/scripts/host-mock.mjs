#!/usr/bin/env node
/**
 * Local host mock / dev gateway for slices 4 and 5.
 *
 * Reads plugin declarations directly from `plugins-v2/` Markdown files and
 * serves:
 *   - POST /v1/auth/session, DELETE /v1/auth/session
 *   - GET  /v1/spaces/{space}/declarations
 *   - POST /v1/spaces/{space}/call   (docs/reviews/host-call-route-2026-11-27.md)
 *   - GET  /v1/spaces/{space}/live   (docs/reviews/live-window-protocol-2026-11-27.md, RFC 6455 WS)
 *
 * Zero external dependencies: uses only Node built-ins (`node:http`, `node:crypto`, `node:fs`).
 */
import http from "node:http";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "../../..");
const PORT = Number(process.env.HOST_MOCK_PORT || 4390);
const PLUGIN_DIR = process.env.HOST_PLUGIN_DIR || path.join(REPO_ROOT, "plugins-v2/calternal-chat");

// --- Generic Markdown frontmatter + declaration parser ---

function parseFrontmatter(text) {
  const m = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/.exec(text);
  if (!m) return { top: {}, blocks: {}, body: text.trim() };
  const top = {};
  const blocks = {};
  let currentBlock = null;
  for (const rawLine of m[1].split(/\r?\n/)) {
    if (!rawLine.trim()) continue;
    if (/^\s{2,}/.test(rawLine) && currentBlock) {
      const idx = rawLine.indexOf(":");
      if (idx === -1) {
        blocks[currentBlock].push({ key: rawLine.trim(), value: "" });
      } else {
        blocks[currentBlock].push({
          key: rawLine.slice(0, idx).trim(),
          value: rawLine.slice(idx + 1).trim(),
        });
      }
    } else {
      const idx = rawLine.indexOf(":");
      if (idx === -1) continue;
      const key = rawLine.slice(0, idx).trim();
      const value = rawLine.slice(idx + 1).trim();
      if (value === "") {
        currentBlock = key;
        blocks[key] = [];
      } else {
        currentBlock = null;
        top[key] = value;
      }
    }
  }
  return { top, blocks, body: m[2].trim() };
}

function parseFieldEntry(key, raw) {
  if (raw.startsWith("=")) {
    return { name: key, type: "number", formula: raw.slice(1).trim() };
  }
  const parts = raw.split(",").map((p) => p.trim());
  const field = { name: key, type: parts[0] || "text" };
  for (const p of parts.slice(1)) {
    if (p === "required") field.required = true;
    else if (p.startsWith("default ")) {
      const v = p.slice(8).trim();
      field.default = v === "yes" ? true : v === "no" ? false : v;
    }
  }
  return field;
}

function parseOperationEntry(key, raw) {
  const parts = raw ? raw.split(",").map((p) => p.trim()).filter(Boolean) : [];
  const placement = [];
  let when;
  let within;
  const rest = [];
  for (const p of parts) {
    if (p === "menu" || p === "primary" || p === "swipe") placement.push(p);
    else if (p.startsWith("when ")) when = p.slice(5).trim();
    else if (p.startsWith("within ")) within = p.slice(7).trim();
    else rest.push(p);
  }
  if (key === "add") {
    return { name: key, form: "add", fields: rest, placement };
  }
  if (key === "change") {
    const fields = rest.map((f) => f.replace(/^only\s+/, "").trim()).filter(Boolean);
    return { name: key, form: "change", fields, when, placement };
  }
  if (key === "remove") {
    return { name: key, form: "remove", fields: [], when, placement };
  }
  if (key === "change record") {
    return { name: key, form: "change_record", fields: rest, when, within, placement };
  }
  if (key === "remove record") {
    return { name: key, form: "remove_record", fields: [], when, placement };
  }
  if (rest[0] === "code") {
    return { name: key, form: "code", fields: [], when, placement };
  }
  if (rest[0]?.startsWith("set ")) {
    const eq = rest[0].slice(4).split("=");
    const target = eq[0]?.trim();
    const rawVal = eq[1]?.trim();
    const value = rawVal === "yes" ? true : rawVal === "no" ? false : rawVal;
    return { name: key, form: "set", fields: [], target, value, when, placement };
  }
  return { name: key, form: "code", fields: rest, when, placement };
}

function parsePluginDir(dir) {
  const pFile = parseFrontmatter(fs.readFileSync(path.join(dir, "plugin.md"), "utf8"));
  const pluginId = pFile.top.plugin;
  const slots = {};
  for (const { key, value } of pFile.blocks.slots || []) {
    const parts = value.split(",").map((s) => s.trim());
    const gives = parts.find((p) => p.startsWith("gives "))?.slice(6).replace(/^"|"$/g, "") || "";
    const recommended = parts.find((p) => p.startsWith("recommended "))?.slice(12).trim();
    slots[key] = { name: key, cardinality: parts[0] === "many" ? "many" : "single", gives, recommended };
  }
  const fills = {};
  for (const { key, value } of pFile.blocks.fills || []) {
    const [kind, ...rest] = value.split(/\s+/);
    fills[key] = { slot: key, kind: kind || "operation", target: rest.join(" ") };
  }

  const kinds = {};
  const kindsDir = path.join(dir, "kinds");
  if (fs.existsSync(kindsDir)) {
    for (const f of fs.readdirSync(kindsDir).filter((n) => n.endsWith(".md"))) {
      const k = parseFrontmatter(fs.readFileSync(path.join(kindsDir, f), "utf8"));
      const kid = k.top.kind;
      const fields = {};
      for (const e of k.blocks.fields || []) fields[e.key] = parseFieldEntry(e.key, e.value);
      const records = {};
      for (const e of k.blocks.records || []) records[e.key] = parseFieldEntry(e.key, e.value);
      const state = {};
      for (const e of k.blocks.state || []) state[e.key] = parseFieldEntry(e.key, e.value);
      const operations = {};
      for (const e of k.blocks.operations || []) operations[e.key] = parseOperationEntry(e.key, e.value);
      const row = (k.top.row || "title").split(",").map((s) => s.trim()).filter(Boolean);
      const page = [];
      if (k.top.page) {
        const rm = /^records\(([^)]*)\)$/.exec(k.top.page.trim());
        if (rm) {
          page.push({ kind: "records", recordFields: rm[1].split(",").map((s) => s.trim()).filter(Boolean) });
        } else {
          page.push({ kind: "body" });
        }
      }
      kinds[kid] = {
        id: kid,
        qualified: `${pluginId}.${kid}`,
        plugin: pluginId,
        name: k.top.name || kid,
        folder: k.top.folder || kid,
        editable: k.top.editable === "operations" ? "operations" : "outside",
        fields,
        records,
        state,
        row,
        page,
        operations,
      };
    }
  }

  const collections = {};
  const colsDir = path.join(dir, "collections");
  if (fs.existsSync(colsDir)) {
    for (const f of fs.readdirSync(colsDir).filter((n) => n.endsWith(".md"))) {
      const c = parseFrontmatter(fs.readFileSync(path.join(colsDir, f), "utf8"));
      const cid = c.top.collection;
      const from = c.top.from?.includes(".") ? c.top.from : `${pluginId}.${c.top.from}`;
      const views = (c.blocks.views || []).map((v) => ({
        type: v.key,
        columns: v.value.split(",").map((s) => s.trim()).filter(Boolean),
      }));
      collections[cid] = {
        id: cid,
        qualified: `${pluginId}.${cid}`,
        plugin: pluginId,
        title: c.top.title || cid,
        from,
        where: c.top.where,
        sort: (c.top.sort || "").split(",").map((s) => s.trim()).filter(Boolean),
        views,
      };
    }
  }

  const sidebars = {};
  const sideDir = path.join(dir, "sidebars");
  if (fs.existsSync(sideDir)) {
    for (const f of fs.readdirSync(sideDir).filter((n) => n.endsWith(".md"))) {
      const s = parseFrontmatter(fs.readFileSync(path.join(sideDir, f), "utf8"));
      const sid = s.top.sidebar;
      const entries = [];
      for (const line of s.body.split(/\r?\n/)) {
        const trimmed = line.trim();
        const lm = /^-\s+\[([^\]]+)\]\(<?([^>)]+)>?\)$/.exec(trimmed);
        if (lm) {
          const cid = path.basename(lm[2], ".md");
          entries.push({ type: "collection", title: lm[1], collection: `${pluginId}.${cid}` });
        } else if (trimmed.startsWith("- section:")) {
          entries.push({ type: "section", title: trimmed.slice(10).trim() });
        }
      }
      sidebars[sid] = { id: sid, plugin: pluginId, title: s.top.title || sid, entries };
    }
  }

  return {
    id: pluginId,
    name: pFile.top.name || pluginId,
    icon: pFile.top.icon || "apps",
    version: pFile.top.version || "1.0.0",
    description: pFile.body,
    slots,
    fills,
    kinds,
    collections,
    sidebars,
  };
}

const installedPlugins = [parsePluginDir(PLUGIN_DIR)];
const SPACE_ID = "019a0000-0000-7000-8000-000000000001";

// --- In-memory space state (items, records, viewer state, receipts) ---

const sessions = new Map(); // token -> { token, account, name, space }
const accountsByName = new Map();
const items = new Map(); // itemId -> { id, kind, fields, body, records, version, edited }
const viewerState = new Map(); // `${account}:${itemId}` -> { pinned, muted, read_through }
const receipts = new Map(); // idempotency_key -> result
const liveSockets = new Set();

function newAnchor() {
  const alphabet = "0123456789abcdefghjkmnpqrstvwxyz";
  const bytes = crypto.randomBytes(16);
  let out = "";
  for (let i = 0; i < 16; i++) out += alphabet[bytes[i] % alphabet.length];
  return out;
}

function getViewerState(account, itemId) {
  return viewerState.get(`${account}:${itemId}`) || { pinned: false, muted: false };
}

function renderRow(account, item) {
  const st = getViewerState(account, item.id);
  const liveRecords = item.records.filter((r) => !r.redacted);
  const readIdx = st["read through"]
    ? liveRecords.findIndex((r) => r.anchor === st["read through"])
    : -1;
  const unread = readIdx >= 0 ? liveRecords.length - readIdx - 1 : 0;
  return {
    id: item.id,
    kind: item.kind,
    fields: { ...item.fields },
    computed: { unread },
    state: { ...st },
    version: item.version,
    edited: item.edited,
  };
}

function matchWhere(where, row) {
  if (!where) return true;
  const w = where.trim();
  if (w === "not archived") return !row.fields.archived;
  if (w === "archived") return Boolean(row.fields.archived);
  if (w === "state.pinned and not archived") return Boolean(row.state?.pinned) && !row.fields.archived;
  if (w.startsWith("count(people) > 2")) {
    const people = Array.isArray(row.fields.people) ? row.fields.people : [];
    return people.length > 2 && !row.fields.archived;
  }
  if (w.startsWith("count(people) = 2")) {
    const people = Array.isArray(row.fields.people) ? row.fields.people : [];
    return people.length === 2 && !row.fields.archived;
  }
  return true;
}

function runQuery(account, input) {
  let from = input.from;
  let where = input.where;
  if (input.collection) {
    const [pid, cid] = input.collection.split(".");
    const plugin = installedPlugins.find((p) => p.id === pid);
    const col = plugin?.collections[cid];
    if (!col) throw { code: "not-found", message: "Collection not found.", next: "Check the collection ID." };
    from = col.from;
    where = col.where;
  }
  const rows = [];
  for (const item of items.values()) {
    if (from && item.kind !== from) continue;
    const row = renderRow(account, item);
    if (matchWhere(where, row)) rows.push(row);
  }
  rows.sort((a, b) => (b.edited || 0) - (a.edited || 0));
  return { rows, totals: {}, cursor: null };
}

function runRead(account, itemId) {
  const item = items.get(itemId);
  if (!item) throw { code: "not-found", message: "Item was not found.", next: "Check the item ID." };
  const row = renderRow(account, item);
  return {
    ...row,
    body: item.body,
    records: item.records.filter((r) => !r.redacted),
  };
}

function executeCall(session, envelope) {
  const { call, input = {}, idempotency_key } = envelope;
  if (idempotency_key && receipts.has(idempotency_key)) {
    return receipts.get(idempotency_key);
  }
  let res;
  let mutated = false;
  let appended = null; // { itemId, record, version }
  let changedItemId = null;

  if (call === "s.me") {
    res = { id: session.account, role: "owner" };
  } else if (call === "s.space") {
    res = { id: SPACE_ID };
  } else if (call === "s.query") {
    res = runQuery(session.account, input);
  } else if (call === "s.read") {
    res = runRead(session.account, input.item);
  } else if (call === "s.state") {
    const key = `${session.account}:${input.item}`;
    const cur = { ...getViewerState(session.account, input.item) };
    if (input.op === "set") {
      cur[input.key] = input.value;
      viewerState.set(key, cur);
      mutated = true;
      changedItemId = input.item;
    }
    res = { value: cur[input.key] ?? null };
  } else if (call === "s.write") {
    const changes = [];
    for (const ch of input.changes || []) {
      if (ch.op === "add") {
        const id = crypto.randomUUID();
        const now = Date.now();
        const item = {
          id,
          kind: ch.kind,
          fields: { archived: false, people: [], ...(ch.fields || {}) },
          body: ch.body || "",
          records: [],
          version: 0,
          edited: now,
        };
        items.set(id, item);
        changes.push({ id, version: 0 });
        mutated = true;
      } else if (ch.op === "change") {
        const item = items.get(ch.item);
        if (!item) throw { code: "not-found", message: "Item not found.", next: "Refresh." };
        Object.assign(item.fields, ch.fields || {});
        if (typeof ch.body === "string") item.body = ch.body;
        item.version += 1;
        item.edited = Date.now();
        changes.push({ id: item.id, version: item.version });
        mutated = true;
        changedItemId = item.id;
      } else if (ch.op === "append record") {
        const item = items.get(ch.item);
        if (!item) throw { code: "not-found", message: "Item not found.", next: "Refresh." };
        const anchor = newAnchor();
        const rec = {
          anchor,
          author: session.account,
          author_name: session.name,
          at: new Date().toISOString(),
          body: ch.record?.body || "",
          fields: ch.record?.fields || {},
        };
        item.records.push(rec);
        item.version += 1;
        item.edited = Date.now();
        changes.push({ id: item.id, version: item.version, anchor, link: `[Record](<${item.id}#${anchor}>)` });
        mutated = true;
        appended = { itemId: item.id, record: rec, version: item.version };
      } else if (ch.op === "change record") {
        const item = items.get(ch.item);
        const rec = item?.records.find((r) => r.anchor === ch.anchor);
        if (!item || !rec) throw { code: "not-found", message: "Record not found.", next: "Refresh." };
        if (typeof ch.record?.body === "string") rec.body = ch.record.body;
        rec.edited = true;
        item.version += 1;
        item.edited = Date.now();
        changes.push({ id: item.id, version: item.version, anchor: rec.anchor });
        mutated = true;
        changedItemId = item.id;
      } else if (ch.op === "remove record") {
        const item = items.get(ch.item);
        const rec = item?.records.find((r) => r.anchor === ch.anchor);
        if (!item || !rec) throw { code: "not-found", message: "Record not found.", next: "Refresh." };
        rec.redacted = true;
        item.version += 1;
        item.edited = Date.now();
        changes.push({ id: item.id, version: item.version, anchor: rec.anchor });
        mutated = true;
        changedItemId = item.id;
      }
    }
    res = { changes };
  } else if (call.endsWith(".rename")) {
    const itemId = input.item || input.chat;
    const item = items.get(itemId);
    if (!item) throw { code: "not-found", message: "Item not found.", next: "Refresh." };
    item.fields.title = String(input.title || "").trim();
    item.version += 1;
    item.edited = Date.now();
    res = { changes: [{ id: item.id, version: item.version }] };
    mutated = true;
    changedItemId = item.id;
  } else if (call.endsWith(".send")) {
    const itemId = input.item || input.chat;
    const text = String(input.text || "");
    if (!text.trim()) throw { code: "invalid", message: "Write a message first.", next: "Edit the message." };
    const item = items.get(itemId);
    if (!item) throw { code: "not-found", message: "Item not found.", next: "Refresh." };
    const anchor = newAnchor();
    const rec = {
      anchor,
      author: session.account,
      author_name: session.name,
      at: new Date().toISOString(),
      body: text,
      fields: input.quote ? { quote: input.quote } : {},
    };
    item.records.push(rec);
    item.version += 1;
    item.edited = Date.now();
    res = { changes: [{ id: item.id, version: item.version, anchor, link: `[Record](<${item.id}#${anchor}>)` }] };
    mutated = true;
    appended = { itemId: item.id, record: rec, version: item.version };
  } else {
    throw { code: "not-allowed", message: `Call ${call} is unavailable.`, next: "Check the call name." };
  }

  if (idempotency_key) receipts.set(idempotency_key, res);
  if (mutated) broadcastLive({ appended, changedItemId });
  return res;
}

// --- Minimal RFC 6455 WebSocket server for GET /v1/spaces/{space}/live ---

function encodeWsText(str) {
  const payload = Buffer.from(str, "utf8");
  const len = payload.length;
  let header;
  if (len < 126) {
    header = Buffer.from([0x81, len]);
  } else if (len < 65536) {
    header = Buffer.alloc(4);
    header[0] = 0x81;
    header[1] = 126;
    header.writeUInt16BE(len, 2);
  } else {
    header = Buffer.alloc(10);
    header[0] = 0x81;
    header[1] = 127;
    header.writeBigUInt64BE(BigInt(len), 2);
  }
  return Buffer.concat([header, payload]);
}

function sendWs(client, obj) {
  if (!client.socket.destroyed) {
    client.socket.write(encodeWsText(JSON.stringify(obj)));
  }
}

function broadcastLive({ appended, changedItemId }) {
  for (const client of liveSockets) {
    if (!client.session) continue;
    for (const [subId, sub] of client.subs) {
      if (sub.type === "query") {
        try {
          const result = runQuery(client.session.account, sub.spec);
          sendWs(client, { type: "rows", sub: subId, ...result });
        } catch {
          // ignore
        }
      } else if (sub.type === "records") {
        if (appended && appended.itemId === sub.item) {
          sub.lastAnchor = appended.record.anchor;
          sendWs(client, {
            type: "record_append",
            sub: subId,
            item: sub.item,
            version: appended.version,
            record: appended.record,
          });
        } else if (changedItemId === sub.item) {
          const detail = runRead(client.session.account, sub.item);
          const tail = detail.records.at(-1)?.anchor ?? null;
          if (detail.records.length !== sub.lastCount) {
            sub.lastCount = detail.records.length;
            sub.lastAnchor = tail;
            sendWs(client, {
              type: "records",
              sub: subId,
              item: sub.item,
              version: detail.version,
              fields: detail.fields,
              computed: detail.computed,
              mode: "snapshot",
              after: null,
              records: detail.records,
              last_anchor: tail,
            });
          } else {
            sendWs(client, {
              type: "item_change",
              sub: subId,
              item: sub.item,
              version: detail.version,
              fields: detail.fields,
              computed: detail.computed,
            });
          }
        }
      }
    }
  }
}

function handleWsFrame(client, text) {
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    return;
  }
  if (msg.type === "ping") {
    sendWs(client, { type: "pong" });
    return;
  }
  if (!client.session) {
    if (msg.type === "auth" && sessions.has(msg.token)) {
      client.session = sessions.get(msg.token);
      sendWs(client, { type: "ready", account: client.session.account, space: SPACE_ID });
    } else {
      sendWs(client, { type: "error", sub: null, code: "needs-sign-in", message: "Sign in first.", next: "Sign in." });
      client.socket.end();
    }
    return;
  }
  if (msg.type === "unsubscribe") {
    client.subs.delete(msg.sub);
    return;
  }
  if (msg.type === "subscribe") {
    if (msg.query) {
      client.subs.set(msg.sub, { type: "query", spec: msg.query });
      const result = runQuery(client.session.account, msg.query);
      sendWs(client, { type: "rows", sub: msg.sub, ...result });
    } else if (msg.records) {
      const detail = runRead(client.session.account, msg.records.item);
      const after = msg.records.after || null;
      let mode = "snapshot";
      let out = detail.records;
      let usedAfter = null;
      if (after) {
        const pos = detail.records.findIndex((r) => r.anchor === after);
        if (pos >= 0) {
          mode = "delta";
          out = detail.records.slice(pos + 1);
          usedAfter = after;
        }
      }
      const tail = detail.records.at(-1)?.anchor ?? null;
      client.subs.set(msg.sub, {
        type: "records",
        item: msg.records.item,
        lastAnchor: tail,
        lastCount: detail.records.length,
      });
      sendWs(client, {
        type: "records",
        sub: msg.sub,
        item: msg.records.item,
        version: detail.version,
        fields: detail.fields,
        computed: detail.computed,
        mode,
        after: usedAfter,
        records: out,
        last_anchor: tail,
      });
    }
  }
}

// --- Seed one welcome item so initial screenshots show real content ---
function seedIfRequested() {
  if (process.env.HOST_MOCK_SEED !== "1") return;
  const id = "019a0000-0000-7000-8000-000000000010";
  const now = Date.now();
  items.set(id, {
    id,
    kind: `${installedPlugins[0].id}.chats`,
    fields: { title: "Dependency audit", archived: false, people: ["Ivo Novak", "Hana Sato"] },
    body: "",
    records: [
      {
        anchor: newAnchor(),
        author: "person-ivo",
        author_name: "Ivo Novak",
        at: new Date(now - 600_000).toISOString(),
        body: "Morning! Did the dependency audit finish overnight?",
        fields: {},
      },
      {
        anchor: newAnchor(),
        author: "person-hana",
        author_name: "Hana Sato",
        at: new Date(now - 300_000).toISOString(),
        body: "Can someone summarise the findings for the release notes?",
        fields: {},
      },
    ],
    version: 2,
    edited: now - 300_000,
  });
}
seedIfRequested();

function readJson(req) {
  return new Promise((resolve, reject) => {
    let raw = "";
    req.on("data", (c) => (raw += c));
    req.on("end", () => {
      try {
        resolve(raw ? JSON.parse(raw) : {});
      } catch (e) {
        reject(e);
      }
    });
  });
}

function sendJson(res, status, obj) {
  res.writeHead(status, {
    "content-type": "application/json",
    "access-control-allow-origin": "*",
    "access-control-allow-headers": "content-type, authorization",
    "access-control-allow-methods": "GET, POST, DELETE, OPTIONS",
  });
  res.end(JSON.stringify(obj));
}

function bearerOf(req) {
  const h = req.headers.authorization || "";
  return h.startsWith("Bearer ") ? h.slice(7) : null;
}

const server = http.createServer(async (req, res) => {
  if (req.method === "OPTIONS") {
    res.writeHead(204, {
      "access-control-allow-origin": "*",
      "access-control-allow-headers": "content-type, authorization",
      "access-control-allow-methods": "GET, POST, DELETE, OPTIONS",
    });
    res.end();
    return;
  }

  const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);
  const pathname = url.pathname;

  if (req.method === "POST" && pathname === "/v1/auth/session") {
    const body = await readJson(req);
    const name = String(body.name || "").trim();
    if (!name) return sendJson(res, 400, { error: { code: "invalid", message: "Name is required.", next: "Enter your name." } });
    let account = accountsByName.get(name);
    if (!account) {
      account = crypto.randomUUID();
      accountsByName.set(name, account);
    }
    const token = crypto.randomBytes(24).toString("hex");
    const session = { token, account, name, space: SPACE_ID };
    sessions.set(token, session);
    return sendJson(res, 200, { session });
  }

  if (req.method === "DELETE" && pathname === "/v1/auth/session") {
    const token = bearerOf(req);
    if (token) sessions.delete(token);
    return sendJson(res, 200, { ok: true });
  }

  const declMatch = /^\/v1\/spaces\/([^/]+)\/declarations$/.exec(pathname);
  if (req.method === "GET" && declMatch) {
    const session = sessions.get(bearerOf(req));
    if (!session) return sendJson(res, 401, { error: { code: "needs-sign-in", message: "Sign in first.", next: "Sign in." } });
    return sendJson(res, 200, { space: SPACE_ID, plugins: installedPlugins });
  }

  const callMatch = /^\/v1\/spaces\/([^/]+)\/call$/.exec(pathname);
  if (req.method === "POST" && callMatch) {
    const session = sessions.get(bearerOf(req));
    if (!session) return sendJson(res, 401, { error: { code: "needs-sign-in", message: "Sign in first.", next: "Sign in." } });
    try {
      const body = await readJson(req);
      const result = executeCall(session, body);
      return sendJson(res, 200, { result });
    } catch (err) {
      const e = err && err.code ? err : { code: "invalid", message: String(err), next: "Check the request." };
      const status = e.code === "not-found" ? 404 : e.code === "not-allowed" ? 403 : 400;
      return sendJson(res, status, { error: e });
    }
  }

  sendJson(res, 404, { error: { code: "not-found", message: "Route not found.", next: "Check the URL." } });
});

server.on("upgrade", (req, socket) => {
  const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);
  if (!/^\/v1\/spaces\/[^/]+\/live$/.test(url.pathname)) {
    socket.destroy();
    return;
  }
  const key = req.headers["sec-websocket-key"];
  if (!key) {
    socket.destroy();
    return;
  }
  const accept = crypto
    .createHash("sha1")
    .update(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11")
    .digest("base64");
  socket.write(
    "HTTP/1.1 101 Switching Protocols\r\n" +
      "Upgrade: websocket\r\n" +
      "Connection: Upgrade\r\n" +
      `Sec-WebSocket-Accept: ${accept}\r\n\r\n`,
  );

  const client = { socket, session: null, subs: new Map() };
  liveSockets.add(client);

  let buf = Buffer.alloc(0);
  socket.on("data", (chunk) => {
    buf = Buffer.concat([buf, chunk]);
    while (buf.length >= 2) {
      const opcode = buf[0] & 0x0f;
      const masked = (buf[1] & 0x80) !== 0;
      let len = buf[1] & 0x7f;
      let offset = 2;
      if (len === 126) {
        if (buf.length < 4) return;
        len = buf.readUInt16BE(2);
        offset = 4;
      } else if (len === 127) {
        if (buf.length < 10) return;
        len = Number(buf.readBigUInt64BE(2));
        offset = 10;
      }
      const maskLen = masked ? 4 : 0;
      if (buf.length < offset + maskLen + len) return;
      const mask = masked ? buf.subarray(offset, offset + 4) : null;
      const payload = Buffer.from(buf.subarray(offset + maskLen, offset + maskLen + len));
      buf = buf.subarray(offset + maskLen + len);
      if (mask) {
        for (let i = 0; i < payload.length; i++) payload[i] ^= mask[i % 4];
      }
      if (opcode === 0x8) {
        socket.end();
        return;
      }
      if (opcode === 0x9) {
        // Pong
        socket.write(Buffer.concat([Buffer.from([0x8a, payload.length]), payload]));
        continue;
      }
      if (opcode === 0x1) {
        handleWsFrame(client, payload.toString("utf8"));
      }
    }
  });
  socket.on("close", () => liveSockets.delete(client));
  socket.on("error", () => liveSockets.delete(client));
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`host-mock listening on http://127.0.0.1:${PORT}`);
});
