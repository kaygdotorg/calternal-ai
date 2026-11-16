import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtemp, readFile, rm, unlink } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve, join } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const serverBinary = process.env.CALTERNA_SERVER_BIN ?? resolve(repoRoot, "target/debug/calternal");
const pluginRoot = resolve(repoRoot, "plugins");
const sleep = (milliseconds) => new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));

function readStartup(dataDir) {
  return readFile(join(dataDir, "startup.json"), "utf8").then((text) => JSON.parse(text));
}

async function startServer(dataDir) {
  const startupPath = join(dataDir, "startup.json");
  await unlink(startupPath).catch(() => {});
  const child = spawn(serverBinary, [
    "serve",
    "--data-dir", dataDir,
    "--port", "0",
    "--plugin-root", pluginRoot,
    "--node", process.execPath,
  ], {
    cwd: repoRoot,
    // The test server gets no ambient credentials or fault-injection settings.
    env: { PATH: process.env.PATH ?? "", LANG: "C.UTF-8", RUST_LOG: "warn" },
    stdio: ["ignore", "ignore", "pipe"],
  });

  const stderrChunks = [];
  let stderrBytes = 0;
  child.stderr.on("data", (chunk) => {
    if (stderrBytes >= 32 * 1024) return;
    const remaining = 32 * 1024 - stderrBytes;
    const kept = chunk.subarray(0, remaining);
    stderrChunks.push(kept);
    stderrBytes += kept.byteLength;
  });

  const deadline = Date.now() + 20_000;
  let startup;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`calternal exited before startup (${child.exitCode}): ${Buffer.concat(stderrChunks).toString("utf8")}`);
    }
    try {
      startup = await readStartup(dataDir);
      if (startup.pid === child.pid) break;
    } catch {
      // startup.json is the readiness signal; do not probe the port.
    }
    await sleep(20);
  }
  if (!startup || startup.pid !== child.pid) {
    await stopServer({ child });
    throw new Error(`calternal did not publish startup.json: ${Buffer.concat(stderrChunks).toString("utf8")}`);
  }

  const ownerToken = await readFile(startup.token_files.owner, "utf8");
  const guestToken = await readFile(startup.token_files.guest, "utf8");
  assert.match(ownerToken, /^[a-f0-9]{64}$/);
  assert.match(guestToken, /^[a-f0-9]{64}$/);
  assert.notEqual(ownerToken, guestToken);
  return {
    child,
    startup,
    baseUrl: startup.base_url,
    ownerToken,
    guestToken,
    stderrChunks,
  };
}

async function stopServer(server) {
  if (!server?.child || server.child.exitCode !== null || server.child.signalCode !== null) return;
  const child = server.child;
  const exited = new Promise((resolvePromise) => child.once("exit", resolvePromise));
  child.kill("SIGTERM");
  const clean = await Promise.race([exited.then(() => true), sleep(5000).then(() => false)]);
  if (!clean) {
    child.kill("SIGKILL");
    await exited;
  }
}

async function operation(server, token, name, input, requestId = randomUUID()) {
  const response = await fetch(`${server.baseUrl}/api/v1/operations`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ request_id: requestId, operation: name, input }),
  });
  return { status: response.status, body: await response.json() };
}

test("Notes and the trusted TypeScript extension work through a disposable real server", { timeout: 60_000 }, async () => {
  const dataDir = await mkdtemp(join(tmpdir(), "calternal-node-integration-"));
  let server;
  try {
    server = await startServer(dataDir);
    assert.equal(server.startup.profile, "development-preview");
    assert.equal(new URL(server.baseUrl).hostname, "127.0.0.1");

    const describe = await fetch(`${server.baseUrl}/api/v1/describe`, {
      headers: { authorization: `Bearer ${server.ownerToken}` },
    });
    assert.equal(describe.status, 200);
    const catalog = await describe.json();
    assert.equal(catalog.protocol_version, 1);
    assert.equal(catalog.genui_version, 1);
    assert.ok(catalog.operations.some((entry) => entry.id === "notes-insights.summarize"));

    const createInput = { space_id: "personal", title: "Integration note", markdown: "Hello world\n" };
    const createRequestId = `create-${randomUUID()}`;
    const create = await operation(server, server.ownerToken, "notes.create", createInput, createRequestId);
    assert.equal(create.status, 200, JSON.stringify(create.body));
    assert.equal(create.body.ok, true);
    const note = create.body.data.note;

    // A retry with the exact principal, request ID, operation and input returns the committed result.
    const retry = await operation(server, server.ownerToken, "notes.create", createInput, createRequestId);
    assert.equal(retry.status, 200, JSON.stringify(retry.body));
    assert.deepEqual(retry.body.data.note, note);

    const list = await operation(server, server.ownerToken, "notes.list", { space_id: "personal" });
    assert.equal(list.status, 200, JSON.stringify(list.body));
    assert.equal(list.body.data.notes.length, 1);
    assert.equal(list.body.data.notes[0].id, note.id);
    assert.equal(Object.hasOwn(list.body.data.notes[0], "markdown"), false);

    const stale = await operation(server, server.ownerToken, "notes.update", {
      space_id: "personal",
      id: note.id,
      base_revision: "stale-revision",
      title: "Must not apply",
      markdown: "lost update\n",
    });
    assert.equal(stale.status, 409);
    assert.equal(stale.body.error.code, "conflict");
    const unchanged = await operation(server, server.ownerToken, "notes.get", { space_id: "personal", id: note.id });
    assert.equal(unchanged.body.data.note.markdown, "Hello world\n");

    const update = await operation(server, server.ownerToken, "notes.update", {
      space_id: "personal",
      id: note.id,
      base_revision: note.revision,
      title: "Renamed integration note",
      markdown: "Hello world\n",
    });
    assert.equal(update.status, 200, JSON.stringify(update.body));
    assert.notEqual(update.body.data.note.revision, note.revision);
    assert.equal(update.body.data.note.title, "Renamed integration note");

    const history = await operation(server, server.ownerToken, "notes.history", { space_id: "personal", id: note.id });
    assert.equal(history.status, 200, JSON.stringify(history.body));
    assert.equal(history.body.data.versions.length, 2);
    assert.ok(history.body.data.versions.every((version) => version.markdown === "Hello world\n"));
    assert.deepEqual(new Set(history.body.data.versions.map((version) => version.title)), new Set(["Integration note", "Renamed integration note"]));

    const personalDenied = await operation(server, server.guestToken, "notes.get", { space_id: "personal", id: note.id });
    assert.equal(personalDenied.status, 403);
    assert.equal(personalDenied.body.error.code, "forbidden");

    const sharedCreate = await operation(server, server.ownerToken, "notes.create", {
      space_id: "shared",
      title: "Shared note",
      markdown: "Visible to the guest\n",
    });
    assert.equal(sharedCreate.status, 200, JSON.stringify(sharedCreate.body));
    const sharedRead = await operation(server, server.guestToken, "notes.get", {
      space_id: "shared",
      id: sharedCreate.body.data.note.id,
    });
    assert.equal(sharedRead.status, 200, JSON.stringify(sharedRead.body));
    assert.equal(sharedRead.body.data.note.markdown, "Visible to the guest\n");

    const pluginsBefore = await operation(server, server.ownerToken, "plugins.list", { space_id: "personal" });
    assert.equal(pluginsBefore.status, 200, JSON.stringify(pluginsBefore.body));
    const notesPlugin = pluginsBefore.body.data.plugins.find((plugin) => plugin.manifest.id === "notes");
    const insightsPlugin = pluginsBefore.body.data.plugins.find((plugin) => plugin.manifest.id === "notes-insights");
    assert.equal(notesPlugin.enabled, true);
    assert.equal(insightsPlugin.enabled, false);

    // Preview uses fixed fixture text and remains available before activation.
    const preview = await operation(server, server.ownerToken, "plugins.preview", {
      space_id: "personal",
      plugin_id: "notes-insights",
    });
    assert.equal(preview.status, 200, JSON.stringify(preview.body));
    assert.equal(preview.body.data.preview, true);
    assert.equal(preview.body.data.view.genui_version, 1);
    assert.equal(preview.body.data.view.root.type, "stack");
    assert.deepEqual(preview.body.data.view.root.children.map((node) => node.type), ["stat", "stat", "stat"]);

    const stillDisabled = await operation(server, server.ownerToken, "plugins.list", { space_id: "personal" });
    assert.equal(stillDisabled.body.data.plugins.find((plugin) => plugin.manifest.id === "notes-insights").enabled, false);
    const summarizeDenied = await operation(server, server.ownerToken, "notes-insights.summarize", { space_id: "personal", id: note.id });
    assert.equal(summarizeDenied.status, 403);
    assert.equal(summarizeDenied.body.error.code, "forbidden");

    const enable = await operation(server, server.ownerToken, "plugins.enable", {
      space_id: "personal",
      plugin_id: "notes-insights",
    });
    assert.equal(enable.status, 200, JSON.stringify(enable.body));
    assert.equal(enable.body.data.plugin.enabled, true);

    const summarize = await operation(server, server.ownerToken, "notes-insights.summarize", { space_id: "personal", id: note.id });
    assert.equal(summarize.status, 200, JSON.stringify(summarize.body));
    assert.deepEqual(summarize.body.data.stats, { words: 2, characters: 12, lines: 1 });
    assert.deepEqual(summarize.body.data.view.root.children.map((node) => node.value), ["2", "12", "1"]);

    const tokenBeforeRestart = server.ownerToken;
    await stopServer(server);
    server = await startServer(dataDir);
    assert.equal(server.ownerToken, tokenBeforeRestart);

    const afterRestart = await operation(server, server.ownerToken, "notes.get", { space_id: "personal", id: note.id });
    assert.equal(afterRestart.status, 200, JSON.stringify(afterRestart.body));
    assert.equal(afterRestart.body.data.note.title, "Renamed integration note");
    assert.equal(afterRestart.body.data.note.markdown, "Hello world\n");
    const enabledAfterRestart = await operation(server, server.ownerToken, "plugins.list", { space_id: "personal" });
    assert.equal(enabledAfterRestart.body.data.plugins.find((plugin) => plugin.manifest.id === "notes-insights").enabled, true);
    const historyAfterRestart = await operation(server, server.ownerToken, "notes.history", { space_id: "personal", id: note.id });
    assert.equal(historyAfterRestart.body.data.versions.length, 2);
  } finally {
    await stopServer(server);
    await rm(dataDir, { recursive: true, force: true });
  }
});
