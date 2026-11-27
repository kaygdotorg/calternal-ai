#!/usr/bin/env node
/**
 * Exercise the real host with the browser data client on a same-origin proxy.
 * The disposable fixture supplies its one-time setup code over a private pipe.
 */
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import { get } from "node:http";
import { createServer as tcpServer } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const WEB = fileURLToPath(new URL("../", import.meta.url));
const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const KIND = "calternal-chat.chats";

async function port() {
  const probe = tcpServer().listen(0, "127.0.0.1");
  await once(probe, "listening");
  const result = probe.address().port;
  await new Promise((resolve) => probe.close(resolve));
  return result;
}

async function run(file, args, options) {
  const child = spawn(file, args, { stdio: "inherit", ...options });
  const [code, signal] = await once(child, "exit");
  assert.equal(code, 0, `${file} exited with ${signal ?? code}`);
}

async function waitForHost(api, host) {
  for (let attempt = 0; attempt < 80; attempt++) {
    assert.equal(host.exitCode, null, "host exited before serving");
    try {
      if ((await fetch(`${api}/v1/auth/status`)).ok) return;
    } catch {
      // The listener starts after auth, runtime, and storage initialization.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("timed out waiting for calternal-host");
}

function upgradeStatus(url, origin, fetchSite) {
  return new Promise((resolve, reject) => {
    const request = get(url, {
      headers: {
        connection: "upgrade", upgrade: "websocket", "sec-websocket-version": "13",
        "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==", origin, "sec-fetch-site": fetchSite,
      },
    }, (response) => {
      response.resume();
      resolve(response.statusCode);
    });
    request.on("upgrade", (response, socket) => {
      socket.destroy();
      resolve(response.statusCode);
    });
    request.on("error", reject);
  });
}

async function browserClient(browser, origin) {
  const context = await browser.newContext();
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("WebAuthn.enable");
  const { authenticatorId } = await cdp.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "internal",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
  await page.goto(`${origin}/browser-chat-fixture`);
  await page.evaluate(async () => {
    const client = await import("/src/host/client.ts");
    window.host = {
      client,
      session: null,
      live: null,
      records: [],
      messages: [],
      connections: [],
      query: null,
      async call(name, input = {}, key) {
        return client.hostCall(this.session, name, input, key);
      },
      subscribe(item) {
        this.live = new client.LiveClient(this.session, (state) => this.connections.push(state));
        this.live.subscribeQuery({ collection: "calternal-chat.chats" }, (rows) => { this.query = rows; });
        this.live.subscribeRecords(item, {
          onSnapshot: (records) => {
            this.messages.push(["snapshot", records.map((record) => record.body)]);
            this.records = records;
          },
          onDelta: (records) => {
            this.messages.push(["delta", records.map((record) => record.body)]);
            this.records.push(...records);
          },
          onAppend: (record) => {
            this.messages.push(["append", [record.body]]);
            this.records.push(record);
          },
          onItemChange: () => {},
        });
      },
    };
  });
  return { context, page, cdp, authenticatorId };
}

async function fixture(directory, origin, setupCode) {
  const backend = await port();
  const api = `http://127.0.0.1:${backend}`;
  const address = `127.0.0.1:${backend}`;
  const bin = process.env.HOST_BIN ?? path.join(ROOT, "target/debug/calternal-host");
  const env = {
    ...process.env,
    CALTERNAL_WORKER_BWRAP: execFileSync("sh", ["-c", "command -v bwrap"], { encoding: "utf8" }).trim(),
    CALTERNAL_WORKER_NODE: process.execPath,
    CALTERNAL_WORKER_SDK: path.join(ROOT, "packages/plugin-sdk"),
  };
  const hostArgs = ["serve", directory, origin, address];
  const host = spawn(bin, hostArgs, { env, stdio: ["ignore", "ignore", "inherit"] });
  const stop = () => host.kill("SIGTERM");
  process.once("exit", stop);
  let vite;
  let browser;
  try {
    await waitForHost(api, host);
    vite = await createServer({
      configFile: false,
      root: WEB,
      logLevel: "error",
      optimizeDeps: { noDiscovery: true, include: [] },
      server: {
        host: "127.0.0.1",
        port: Number(new URL(origin).port),
        strictPort: true,
        hmr: false,
        proxy: { "/v1": { target: api, ws: true } },
      },
      plugins: [{
        name: "host-chat-fixture",
        configureServer(server) {
          server.middlewares.use("/browser-chat-fixture", (_req, res) => {
            res.setHeader("content-type", "text/html; charset=utf-8");
            res.end("<!doctype html><title>Browser Chat data client</title>");
          });
        },
      }],
    });
    await vite.listen();
    browser = await chromium.launch({ headless: true });
    const a = await browserClient(browser, origin);
    const session = await a.page.evaluate(async (code) => {
      const { client } = window.host;
      const status = await client.sessionStatus();
      if (!status.setup || status.session) throw new Error("expected fresh host");
      await client.setupHost(code, "Ada");
      await client.signOutHost();
      const signedOut = await client.sessionStatus();
      if (signedOut.setup || signedOut.session) throw new Error("expected revoked session");
      window.host.session = await client.signInHost();
      return window.host.session;
    }, setupCode);
    assert.deepEqual(await a.page.evaluate(() => window.host.client.sessionStatus()), {
      setup: false, session,
    });
    assert.equal(Object.hasOwn(session, "token"), false);
    const cookie = (await a.context.cookies()).find(({ name }) => name === "__Host-calternal_session");
    assert.ok(cookie?.httpOnly && cookie.secure && cookie.sameSite === "Strict" && cookie.path === "/",
      "expected strict host-only browser cookie");
    assert.equal(await a.page.evaluate(() => document.cookie.includes("__Host-calternal_session")), false);

    const hostile = await a.context.request.post(`${origin}/v1/auth/logout`, {
      headers: { origin: "http://foreign.invalid", "sec-fetch-site": "same-site" },
      data: {},
    });
    assert.equal(hostile.status(), 403);
    assert.equal(hostile.headers()["cache-control"], "no-store");
    const missingOrigin = await a.context.request.post(`${origin}/v1/auth/logout`, { data: {} });
    assert.equal(missingOrigin.status(), 403);
    const liveUrl = `${api}/v1/spaces/${session.space}/live`;
    assert.equal(await upgradeStatus(liveUrl, "http://foreign.invalid", "same-site"), 403);
    assert.equal(await upgradeStatus(liveUrl, origin, "same-origin"), 401);

    await run(bin, ["install", directory, origin, session.account, session.space,
      path.join(ROOT, "plugins-v2/calternal-chat"), "--official"], { env });

    const item = await a.page.evaluate(async (kind) => {
      const h = window.host;
      const { plugins } = await h.client.fetchDeclarations(h.session);
      const chat = plugins.find((plugin) => plugin.id === "calternal-chat");
      if (chat.kinds.chats.operations.send.inputs.item.type !== "link(chats)"
        || chat.slots.composer.fillers[0].target !== "send") {
        throw new Error("expected installed Chat declarations");
      }
      const created = await h.call("s.write", { changes: [{ op: "add", kind, fields: { title: "Release prep" } }] });
      const item = created.changes[0].id;
      await h.call(`${kind}.send`, { item, text: "First message" }, "send-initial");
      await h.call(`${kind}.rename`, { item, title: "Release prep v2" }, "rename");
      return item;
    }, KIND);
    console.log("PASS passkey enrollment, logout, sign-in, session status, and cookie/origin checks");
    console.log("PASS compiled declarations, create chat, send, and rename");

    // Simulate access to the same synced passkey in an independent context.
    const b = await browserClient(browser, origin);
    const { credentials } = await a.cdp.send("WebAuthn.getCredentials", { authenticatorId: a.authenticatorId });
    assert.equal(credentials.length, 1);
    await b.cdp.send("WebAuthn.addCredential", { authenticatorId: b.authenticatorId, credential: credentials[0] });
    await b.page.evaluate(async (item) => {
      const h = window.host;
      h.session = await h.client.signInHost();
      h.subscribe(item);
    }, item);
    await b.page.waitForFunction(() => window.host.records.length === 1 && window.host.query?.length === 1);
    assert.equal(await b.page.evaluate(() => window.host.query[0].fields.title), "Release prep v2");
    assert.equal(await b.page.evaluate(() => window.host.query[0].when.pin), true);
    assert.equal(await b.page.evaluate(() => window.host.query[0].state.pinned), false);
    await a.page.evaluate((item) => window.host.call("s.state", {
      op: "set", item, key: "pinned", value: true,
    }), item);
    await b.page.waitForFunction(() => window.host.query[0].state.pinned && window.host.query[0].when.unpin);

    const send = (text) => a.page.evaluate(({ kind, item, text }) =>
      window.host.call(`${kind}.send`, { item, text }, text), { kind: KIND, item, text });
    await send("Second message");
    await b.page.waitForFunction(() => window.host.messages.some(([mode]) => mode === "append"));
    const anchor = await b.page.evaluate(() => window.host.records.at(-1).anchor);
    assert.ok(anchor);

    // Hold the second context offline until both appends have committed.
    await b.context.setOffline(true);
    await b.page.evaluate(() => window.host.live.forceReconnect());
    await b.page.waitForFunction(() => window.host.connections.at(-1) === "reconnecting");
    await send("Third message");
    await send("Fourth message");
    await b.context.setOffline(false);
    await b.page.waitForFunction(() => window.host.messages.some(([mode]) => mode === "delta"));
    const seen = await b.page.evaluate(() => window.host.records.map(({ body, anchor }) => ({ body, anchor })));
    assert.deepEqual(seen.map(({ body }) => body), ["First message", "Second message", "Third message", "Fourth message"]);
    assert.equal(new Set(seen.map((record) => record.anchor)).size, 4);
    assert.equal(seen[1].anchor, anchor);
    const frames = await b.page.evaluate(() => window.host.messages);
    assert.deepEqual(frames, [
      ["snapshot", ["First message"]],
      ["append", ["Second message"]],
      ["delta", ["Third message", "Fourth message"]],
    ]);
    await b.page.evaluate(() => window.host.client.signOutHost());
    await b.page.waitForFunction(() => window.host.connections.at(-1) === "reconnecting");
    assert.equal((await b.page.evaluate(() => window.host.client.sessionStatus())).session, null);
    assert.equal((await a.page.evaluate(() => window.host.client.sessionStatus())).session.account, session.account);
    console.log("PASS two isolated contexts, live append, anchored reconnect, and socket revocation");
  } finally {
    process.removeListener("exit", stop);
    await browser?.close();
    await vite?.close();
    if (host.exitCode === null && host.signalCode === null) {
      const exited = once(host, "exit");
      host.kill("SIGTERM");
      await exited;
    }
  }
}

if (process.argv[2] === "--fixture") {
  const setupCode = await new Promise((resolve, reject) => {
    let value = "";
    process.stdin.setEncoding("utf8").on("data", (chunk) => { value += chunk; })
      .on("end", () => resolve(value)).on("error", reject);
  });
  await fixture(process.argv[3], process.argv[4], setupCode);
} else {
  const origin = `http://localhost:${await port()}`;
  await run("cargo", ["run", "--quiet", "-p", "calternal-host", "--example", "browser-chat-fixture", "--", origin], { cwd: ROOT });
}
