// Real-host run socket and restart proof with a disposable passkey account.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer as tcpServer } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import { runKey, runText, slot, verifyRunLog } from "./host-run-log.mjs";

const web = fileURLToPath(new URL("../", import.meta.url));
const root = fileURLToPath(new URL("../../../", import.meta.url));

async function port() {
  const probe = tcpServer().listen(0, "127.0.0.1");
  await once(probe, "listening");
  const number = probe.address().port;
  await new Promise((resolve) => probe.close(resolve));
  return number;
}

async function run(file, args, env = process.env) {
  const child = spawn(file, args, { cwd: root, env, stdio: "inherit" });
  assert.equal((await once(child, "exit"))[0], 0, `${args[0]} failed`);
}

async function ready(api, host) {
  for (let attempt = 0; attempt < 80; attempt++) {
    assert.equal(host.exitCode, null, "host exited during startup");
    try {
      if ((await fetch(`${api}/v1/auth/status`)).ok) return;
    } catch {
      // The listener opens after the Podman restart sweep.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("host did not listen");
}

async function fixture(directory, origin, code) {
  const image = process.env.CALTERNAL_TEST_PROVIDER_IMAGE;
  assert.ok(image, "Build and select the fake provider image first");
  const address = `127.0.0.1:${await port()}`;
  const api = `http://${address}`;
  const bin = process.env.HOST_BIN ?? path.join(root, "target/debug/calternal-host");
  const bwrap = process.env.PATH.split(path.delimiter).map((dir) => path.join(dir, "bwrap"));
  const { accessSync } = await import("node:fs");
  const launcher = bwrap.find((candidate) => {
    try { accessSync(candidate); return true; } catch { return false; }
  });
  assert.ok(launcher, "Run in nix develop");
  const env = {
    ...process.env,
    CALTERNAL_RUN_IMAGE: image,
    CALTERNAL_WORKER_BWRAP: launcher,
    CALTERNAL_WORKER_NODE: process.execPath,
    CALTERNAL_WORKER_SDK: path.join(root, "packages/plugin-sdk"),
  };
  const start = () => spawn(bin, ["serve", directory, origin, address], { env, stdio: ["ignore", "ignore", "inherit"] });
  let host = start();
  async function stop() {
    if (host.exitCode !== null || host.signalCode !== null) return;
    const exited = once(host, "exit");
    host.kill("SIGTERM");
    await exited;
  }
  let server;
  let browser;
  try {
    await ready(api, host);
    server = await createServer({
      configFile: false,
      root: web,
      logLevel: "error",
      optimizeDeps: { noDiscovery: true, include: [] },
      server: {
        host: "127.0.0.1", port: Number(new URL(origin).port), strictPort: true,
        hmr: false, proxy: { "/v1": { target: api, ws: true } },
      },
      plugins: [{
        name: "host-run-fixture",
        configureServer(server) {
          server.middlewares.use("/run-fixture", (_request, response) => {
            response.setHeader("content-type", "text/html; charset=utf-8");
            response.end("<!doctype html><title>Host run socket fixture</title>");
          });
        },
      }],
    });
    await server.listen();
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext();
    const page = await context.newPage();
    const cdp = await context.newCDPSession(page);
    await cdp.send("WebAuthn.enable");
    await cdp.send("WebAuthn.addVirtualAuthenticator", {
      options: {
        protocol: "ctap2", transport: "internal", hasResidentKey: true,
        hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true,
      },
    });
    await page.goto(`${origin}/run-fixture`);
    const session = await page.evaluate(async (code) => {
      const client = await import("/src/host/client.ts");
      await client.setupHost(code, "Run owner");
      const { session } = await client.sessionStatus();
      window.host = { client, session };
      return session;
    }, code);

    for (const plugin of ["calternal-chat", "calternal-chat-claude-code"]) {
      await run(bin, ["install", directory, origin, session.account, session.space,
        path.join(root, "plugins-v2", plugin), "--official"], env);
    }
    const item = await page.evaluate(async () => {
      const { client, session } = window.host;
      const result = await client.hostCall(session, "s.write", {
        changes: [{ op: "add", kind: "calternal-chat.chats", fields: { title: "Run log" } }],
      });
      return result.changes[0].id;
    });
    const runId = await verifyRunLog(page, item);
    console.log("PASS real harness log ordering, Stop, and offset replay after reconnect");

    await stop();
    host = start();
    await ready(api, host);
    const replay = await page.evaluate(async ({ item, runKey, runText, slot }) => {
      const response = await fetch(`/v1/spaces/${window.host.session.space}/call`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          call: "calternal-chat.chats.send", input: { item, text: runText },
          idempotency_key: runKey, slot_selection: slot,
        }),
      });
      if (!response.ok) throw new Error(`replay HTTP ${response.status}`);
      return (await response.json()).result.run;
    }, { item, runKey, runText, slot });
    assert.deepEqual(replay, { id: runId, status: "stopped" });
    console.log("PASS a host restart reports the original run as stopped on retry");
  } finally {
    await browser?.close();
    await server?.close();
    await stop();
  }
}

if (process.argv[2] === "--fixture") {
  process.stdin.setEncoding("utf8");
  let code = "";
  for await (const chunk of process.stdin) code += chunk;
  await fixture(process.argv[3], process.argv[4], code);
} else {
  const origin = `http://localhost:${await port()}`;
  await run("cargo", ["run", "--quiet", "-p", "calternal-host", "--example",
    "browser-chat-fixture", "--", origin, "runs"]);
}
