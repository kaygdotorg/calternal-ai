// Opt-in, single-turn subscription proof. Run with a built calternal-host, web
// client, and plugin bundles under nix develop. The disposable host owns its port.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { slot } from "./host-run-log.mjs";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const prompt = "Reply with exactly one word: pong";
const image = process.env.CALTERNAL_REAL_RUN_IMAGE;
const signIn = process.env.CALTERNAL_RUN_SIGN_IN_DIR;

if (!image || !signIn) {
  console.log("SKIP: set CALTERNAL_REAL_RUN_IMAGE and CALTERNAL_RUN_SIGN_IN_DIR");
} else {
  await smoke();
}

async function smoke() {
  const bin = process.env.HOST_BIN ?? path.join(root, "target/debug/calternal-host");
  const bwrap = execFileSync("sh", ["-c", "command -v bwrap"], { encoding: "utf8" }).trim();
  const directory = await mkdtemp(path.join(tmpdir(), "calternal-real-turn-"));
  const bundles = ["calternal-chat", "calternal-chat-claude-code"]
    .map((plugin) => path.join(root, "plugins-v2", plugin));
  const refused = new Set();
  const traffic = Object.create(null);
  let host, exited, browser, result;
  let phase = "start disposable host";
  try {
    host = spawn(bin, ["dev-serve", directory, path.join(root, "apps/web/dist"), ...bundles], {
      env: {
        PATH: process.env.PATH,
        HOME: process.env.HOME,
        XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
        CALTERNAL_REAL_RUN_IMAGE: image,
        CALTERNAL_RUN_SIGN_IN_DIR: signIn,
        CALTERNAL_WORKER_BWRAP: bwrap,
        CALTERNAL_WORKER_NODE: process.execPath,
        CALTERNAL_WORKER_SDK: path.join(root, "packages/plugin-sdk"),
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    exited = once(host, "close");
    const origin = await new Promise((resolve, reject) => {
      const deadline = setTimeout(reject, 15_000);
      const finish = (value) => { clearTimeout(deadline); resolve(value); };
      createInterface({ input: host.stdout }).on("line", (line) => {
        const ready = /^Chat host (http:\/\/localhost:\d+)\/host$/.exec(line);
        if (ready) finish(ready[1]);
      });
      createInterface({ input: host.stderr }).on("line", (line) => {
        const denied = /^harness egress refused (?:undeclared host|unsafe DNS answer for) ([a-z0-9.-]+)$/.exec(line);
        if (denied) refused.add(denied[1]);
        const bytes = /^harness egress bytes ([a-z0-9.-]+) (to_upstream|to_run) (\d+)$/.exec(line);
        if (bytes) {
          const [, name, direction, count] = bytes;
          traffic[name] ??= { to_upstream: 0, to_run: 0 };
          traffic[name][direction] += Number(count);
        }
      });
      exited.then(() => { clearTimeout(deadline); reject(); }, reject);
    });
    phase = "enroll virtual passkey";
    browser = await chromium.launch();
    const context = await browser.newContext();
    const page = await context.newPage();
    const cdp = await context.newCDPSession(page);
    await cdp.send("WebAuthn.enable");
    await cdp.send("WebAuthn.addVirtualAuthenticator", {
      options: { protocol: "ctap2", transport: "internal", hasResidentKey: true,
        hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true },
    });
    await page.goto(`${origin}/v1/auth/status`);
    const code = await readFile(path.join(directory, "first-owner-code"), "utf8");
    const space = await page.evaluate(async (token) => {
      async function post(route, input) {
        const response = await fetch(route, {
          method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(input),
        });
        if (!response.ok) throw new Error("enrollment rejected");
        return response.json();
      }
      const start = await post("/v1/auth/setup/start", { token, name: "Disposable provider test" });
      const publicKey = PublicKeyCredential.parseCreationOptionsFromJSON(start.options.publicKey);
      const credential = await navigator.credentials.create({ publicKey });
      await post("/v1/auth/setup/finish", { ceremony: start.ceremony, credential: credential.toJSON() });
      return (await (await fetch("/v1/auth/status")).json()).session.space;
    }, code);
    phase = "send one prompt and await the reply";
    result = await page.evaluate(singleTurn, { space, prompt, slot });
  } catch {
    result = { passed: false, phase };
  } finally {
    await browser?.close();
    if (host && host.exitCode === null && host.signalCode === null) host.kill("SIGTERM");
    await exited?.catch(() => {});
    await rm(directory, { recursive: true, force: true });
  }

  if (result.stderr) {
    result.stderr = safeStderr(Buffer.concat(result.stderr.map((chunk) => Buffer.from(chunk, "base64")))
      .toString("utf8"));
  }
  console.log(JSON.stringify({ ...result, refused: [...refused].sort(), traffic }));
  assert.ok(result.passed, "No run-stamped reply and streamed chunk within 120 seconds");
}

// Preserve short, known diagnostic words. Suppress arbitrary strings, including
// identifiers, paths, URLs, credentials, and headers, even when split across chunks.
function safeStderr(stderr) {
  const words = new Set(("error warning failed failure cannot could not connect connection refused " +
    "authentication authenticated login sign in signed token refresh invalid missing expired denied " +
    "permission read only filesystem host network timeout timed out tls certificate api cli adapter " +
    "eacces eperm enoent erofs econnrefused enetunreach etimedout").split(" "));
  return (stderr.match(/\S+/g) ?? []).slice(0, 100).map((word) => {
    const plain = word.replace(/[,:.!]+$/, "").toLowerCase();
    return words.has(plain) ? plain : "[redacted]";
  }).join(" ");
}

async function singleTurn({ space, prompt, slot }) {
  const diagnostics = { passed: false, firstChunkMs: null, replyMs: null, exit: null,
    stderr: [], adapterErrors: [], signedIn: null, outcome: null, pong: false };
  async function call(name, input, extra = {}, signal) {
    const response = await fetch(`/v1/spaces/${space}/call`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ call: name, input, ...extra }), signal,
    });
    if (!response.ok) throw new Error(name);
    return (await response.json()).result;
  }
  const catalog = await call("s.declarations", {});
  if (!["calternal-chat", slot.filler].every((id) => catalog.plugins.some((plugin) => plugin.id === id))) {
    return { ...diagnostics, failure: "plugin installation" };
  }
  const created = await call("s.write", {
    changes: [{ op: "add", kind: "calternal-chat.chats", fields: { title: "Disposable provider test" } }],
  });
  const item = created.changes[0].id;
  const start = performance.now();
  const elapsed = () => Math.round(performance.now() - start);
  const wait = (time) => new Promise((resolve) => setTimeout(resolve, time));
  const decoder = new TextDecoder();
  let pending = "";
  let socket, run;
  let stderrSize = 0;
  const signal = AbortSignal.timeout(120_000);
  try {
    ({ run } = await call("calternal-chat.chats.send", { item, text: prompt }, {
      idempotency_key: "one-real-turn", slot_selection: slot,
    }, signal));
    if (!run?.id) return { ...diagnostics, failure: "run did not start" };
    socket = new WebSocket(`ws://${location.host}/v1/spaces/${space}/live`);
    socket.onopen = () => socket.send(JSON.stringify({
      type: "subscribe", sub: "run", run_log: { run: run.id, after: 0 },
    }));
    socket.onmessage = ({ data }) => {
      const frame = JSON.parse(data);
      if (frame.type !== "run_log") return;
      for (const chunk of frame.chunks) {
        if (chunk.event === "exit") diagnostics.exit = { code: chunk.code, signal: chunk.signal };
        if (chunk.event !== "data") continue;
        diagnostics.firstChunkMs ??= elapsed();
        if (chunk.stream === "stderr" && stderrSize + chunk.bytes.length <= 8192) {
          diagnostics.stderr.push(chunk.bytes);
          stderrSize += chunk.bytes.length;
        }
        if (chunk.stream !== "stdout") continue;
        pending += decoder.decode(Uint8Array.from(atob(chunk.bytes), (ch) => ch.charCodeAt(0)), { stream: true });
        const lines = pending.split("\n");
        pending = (lines.pop() ?? "").slice(-262144);
        for (const line of lines) {
          try {
            const { event } = JSON.parse(line);
            if (event?.type === "check") diagnostics.signedIn = event.signed_in === true;
            if (event?.type === "done") {
              const known = ["done", "failed", "cancelled", "outcome_unknown"];
              diagnostics.outcome = known.includes(event.outcome) ? event.outcome : "other";
            }
            if (event?.type === "error") {
              const known = ["cli_exited", "provider_stream", "authentication_failed", "rate_limit",
                "oauth_org_not_allowed", "server_error", "invalid_request", "model_not_found", "billing_error"];
              diagnostics.adapterErrors.push(known.includes(event.code) ? event.code : "other");
            }
          } catch {}
        }
      }
    };
    while (!signal.aborted) {
      const state = await call("s.read", { item }, {}, signal);
      const reply = state.records.find((record) => record.run === run.id);
      if (reply && diagnostics.replyMs === null) {
        diagnostics.replyMs = elapsed();
        diagnostics.pong = reply.body.trim() === "pong";
      }
      if (diagnostics.replyMs !== null && diagnostics.firstChunkMs !== null) {
        diagnostics.passed = true;
        break;
      }
      if (diagnostics.exit || (diagnostics.outcome && diagnostics.outcome !== "done")) break;
      await wait(150);
    }
  } catch {
    diagnostics.failure = signal.aborted ? "120-second deadline" : "host call failed";
  } finally {
    if (run?.id && !diagnostics.exit) {
      diagnostics.stoppedForCleanup = true;
      await call("s.run.stop", { id: run.id }, {}, AbortSignal.timeout(3000)).catch(() => {});
      for (let attempt = 0; !diagnostics.exit && attempt < 20; attempt++) await wait(50);
    }
    socket?.close();
  }
  return diagnostics;
}
