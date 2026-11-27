// Prove localhost cookies in each browser against real server-verified passkeys.
// The Rust fixture supplies only a disposable enrollment grant on stdin.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, webkit } from "playwright";

const root = fileURLToPath(new URL("../../../", import.meta.url));

async function virtualPasskey(context) {
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("WebAuthn.enable");
  await cdp.send("WebAuthn.addVirtualAuthenticator", {
    options: { protocol: "ctap2", transport: "internal", hasResidentKey: true,
      hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true },
  });
  return page;
}

function credential(page, mode, options) {
  return page.evaluate(async ({ mode, options }) => {
    const response = mode === "setup"
      ? await navigator.credentials.create({
          publicKey: PublicKeyCredential.parseCreationOptionsFromJSON(options.publicKey),
        })
      : await navigator.credentials.get({
          publicKey: PublicKeyCredential.parseRequestOptionsFromJSON(options.publicKey),
        });
    return response.toJSON();
  }, { mode, options });
}

function post(page, url, input) {
  return page.evaluate(async ({ url, input }) => {
    const response = await fetch(url, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    });
    return { status: response.status, data: response.status === 204 ? null : await response.json() };
  }, { url, input });
}

async function ceremony(page, signer, mode, input) {
  const start = await post(page, `/v1/auth/${mode}/start`, input);
  assert.equal(start.status, 200, `${mode} start`);
  const signed = await credential(signer, mode, start.data.options);
  const response = page.waitForResponse((response) => response.url().endsWith(`/v1/auth/${mode}/finish`));
  const finished = await post(page, `/v1/auth/${mode}/finish`, { ceremony: start.data.ceremony, credential: signed });
  assert.equal(finished.status, 200, `${mode} finish`);
  const cookie = await (await response).headerValue("set-cookie");
  for (const part of ["__Host-calternal_session=", "Path=/", "Secure", "HttpOnly", "SameSite=Strict"]) {
    assert.ok(cookie.includes(part), `missing cookie attribute ${part}`);
  }
  return finished.data.session;
}

async function fixture(directory, origin, code, browserName) {
  const port = new URL(origin).port;
  const bin = process.env.HOST_BIN ?? path.join(root, "target/debug/calternal-host");
  const bundles = ["calternal-chat", "calternal-chat-claude-code"].map((id) => path.join(root, "plugins-v2", id));
  const env = {
    ...process.env,
    CALTERNAL_WORKER_BWRAP: execFileSync("sh", ["-c", "command -v bwrap"], { encoding: "utf8" }).trim(),
    CALTERNAL_WORKER_NODE: process.execPath,
    CALTERNAL_WORKER_SDK: path.join(root, "packages/plugin-sdk"),
  };
  const chromiumBrowser = await chromium.launch();
  const browser = browserName === "chromium" ? chromiumBrowser : await webkit.launch();
  const context = await browser.newContext();
  const signer = await virtualPasskey(browserName === "chromium" ? context : await chromiumBrowser.newContext());
  const page = browserName === "chromium" ? signer : await context.newPage();
  let host;
  async function start(args) {
    host = spawn(bin, args, { env, stdio: ["ignore", "ignore", "inherit"] });
    for (let attempt = 0; attempt < 80; attempt++) {
      assert.equal(host.exitCode, null, "host exited");
      try {
        if ((await fetch(`${origin}/v1/auth/status`)).ok) return;
      } catch {}
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("host did not start");
  }
  async function stop() {
    if (!host || host.exitCode !== null || host.signalCode !== null) return;
    const exited = once(host, "exit");
    host.kill("SIGTERM");
    await exited;
  }
  const status = () => page.evaluate(() => fetch("/v1/auth/status").then((r) => r.json()));

  try {
    await start(["serve", directory, origin, `127.0.0.1:${port}`]);
    await page.goto(`${origin}/v1/auth/status`);
    if (signer !== page) await signer.goto(`${origin}/v1/auth/status`);
    const capabilities = await page.evaluate(() => ({
      secure: isSecureContext, WebAuthn: typeof PublicKeyCredential === "function",
    }));
    console.log(`${browserName} localhost ${JSON.stringify(capabilities)}`);
    const session = await ceremony(page, signer, "setup", { token: code, name: "Preview test" });
    const hasCookie = (await status()).session?.account === session.account;
    if (hasCookie) assert.equal((await post(page, "/v1/auth/logout", {})).status, 204);
    assert.equal((await status()).session, null);
    const signedIn = await ceremony(page, signer, "login", {});
    assert.equal(signedIn.account, session.account);
    const sentCookie = (await status()).session?.account === session.account;
    assert.equal(sentCookie, hasCookie, "setup and login cookies must agree");
    if (browserName === "chromium") assert.ok(sentCookie, "Chromium did not send its Secure cookie to localhost");
    console.log(`${browserName} PASS setup and sign-in verified by host, Secure cookie ${sentCookie ? "sent" : "refused"}${signer === page ? "" : " (Chromium virtual authenticator supplied WebAuthn signatures)"}`);
    if (!sentCookie) return;

    const previewArgs = ["preview-serve", directory, path.join(root, "apps/web/dist"), ...bundles, port];
    await stop();
    await start(previewArgs);
    assert.equal((await page.goto(`${origin}/host`)).status(), 200);
    assert.equal((await status()).session.account, session.account);
    assert.equal((await post(page, "/v1/auth/logout", {})).status, 204);
    assert.equal((await ceremony(page, signer, "login", {})).account, session.account);
    const route = `/v1/spaces/${session.space}/call`;
    const catalog = await post(page, route, { call: "s.declarations", input: {} });
    assert.deepEqual(catalog.data.result.plugins.map((plugin) => plugin.id).sort(),
      ["calternal-chat", "calternal-chat-claude-code"]);
    const created = await post(page, route, { call: "s.write", input: {
      changes: [{ op: "add", kind: "calternal-chat.chats", fields: { title: "Persistent test" } }],
    } });
    assert.equal(created.status, 200);
    const item = created.data.result.changes[0].id;
    await stop();
    await start(previewArgs);
    assert.equal((await status()).session.account, session.account);
    assert.equal((await post(page, "/v1/auth/logout", {})).status, 204);
    assert.equal((await ceremony(page, signer, "login", {})).account, session.account);
    const chat = await post(page, route, { call: "s.read", input: { item } });
    assert.equal(chat.data.result.fields.title, "Persistent test");
    console.log(`${browserName} PASS session, passkey, and Chat survive preview restart`);
  } finally {
    if (browser !== chromiumBrowser) await browser.close();
    await chromiumBrowser.close();
    await stop();
  }
}

if (process.argv[2] === "--fixture") {
  process.stdin.setEncoding("utf8");
  let code = "";
  for await (const chunk of process.stdin) code += chunk;
  await fixture(process.argv[3], process.argv[4], code, process.env.PREVIEW_TEST_BROWSER);
} else {
  for (const browser of ["chromium", "webkit"]) {
    const requested = Number(process.env.PREVIEW_TEST_PORT ?? 0);
    assert.notEqual(requested, 4313);
    const probe = createServer().listen(requested, "127.0.0.1");
    await once(probe, "listening");
    const port = probe.address().port;
    await new Promise((resolve) => probe.close(resolve));
    assert.notEqual(port, 4313);
    execFileSync("cargo", ["run", "--locked", "--quiet", "-p", "calternal-host", "--example",
      "browser-chat-fixture", "--", `http://localhost:${port}`, "preview-auth"],
    { cwd: root, env: { ...process.env, PREVIEW_TEST_BROWSER: browser }, stdio: "inherit" });
  }
}
