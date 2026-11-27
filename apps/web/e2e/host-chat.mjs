#!/usr/bin/env node
/**
 * End-to-end Chromium test driving the real `/host` UI screens against the real
 * `calternal-host` binary on a same-origin loopback proxy.
 *
 * Covers:
 * - First-owner passkey enrollment, one-time recovery codes, ADR 0030 sign-out,
 *   and discoverable passkey sign-in;
 * - Cookie and HTTP/WebSocket origin guards;
 * - Installing `plugins-v2/calternal-chat` and a disposable harness slot filler;
 * - Creating a chat, sending a message with a harness filler chosen via the
 *   composer slot picker, renaming via `operation.inputs`, and verifying menu
 *   items hidden/shown by server-evaluated `row.when`;
 * - Streaming reply + Stop control surface (local fixture);
 * - Two independent browser contexts: live append, offline reconnect from the
 *   last anchor without duplicates, and sign-out revocation;
 * - Desktop (1280x800) and phone (390x844) screenshots in light and dark mode
 *   for both list and open-chat states (verifying bottom strip hidden on phone
 *   while a chat is open).
 */
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import fs from "node:fs";
import { get } from "node:http";
import { createServer as tcpServer } from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { preview } from "vite";
import { enterSetupCode, previewReady, previewUnit, service, startPreview } from "./host-preview-service.mjs";

const WEB = fileURLToPath(new URL("../", import.meta.url));
const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const SHOT_DIR = process.env.HOST_SHOT_DIR || "/tmp/browser-chat-shots";

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
    const request = get(
      url,
      {
        headers: {
          connection: "upgrade",
          upgrade: "websocket",
          "sec-websocket-version": "13",
          "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
          origin,
          "sec-fetch-site": fetchSite,
        },
      },
      (response) => {
        response.resume();
        resolve(response.statusCode);
      },
    );
    request.on("upgrade", (response, socket) => {
      socket.destroy();
      resolve(response.statusCode);
    });
    request.on("error", reject);
  });
}

function writeHarnessFixture(dir) {
  fs.writeFileSync(
    path.join(dir, "plugin.md"),
    [
      "---",
      "plugin: test-harness",
      "name: Test Harness",
      "version: 1.0.0",
      "format: 1",
      "fills:",
      '  calternal-chat:harness: operation runs.start, as "Test Harness"',
      "---",
      "Disposable test filler for the harness slot.",
    ].join("\n") + "\n",
  );
  fs.mkdirSync(path.join(dir, "kinds"), { recursive: true });
  fs.writeFileSync(
    path.join(dir, "kinds/runs.md"),
    [
      "---",
      "kind: runs",
      "name: Run",
      "format: 1",
      "folder: Test Harness",
      "editable: operations",
      "fields:",
      "  title: text, required",
      "operations:",
      "  add: title",
      "  start: code",
      "---",
      "Records slot-selected invocations.",
    ].join("\n") + "\n",
  );
  fs.writeFileSync(
    path.join(dir, "operations.ts"),
    [
      'import { on } from "@calternal/plugin-sdk";',
      'export const start = on.call("runs.start", async ({ prompt }: { prompt: string }, s) =>',
      '  s.write([{ add: { kind: "test-harness.runs", fields: { title: prompt } } }]));',
    ].join("\n") + "\n",
  );
}

async function openContext(browser, viewport = { width: 1280, height: 800 }) {
  const context = await browser.newContext({ viewport });
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
  return { context, page, cdp, authenticatorId };
}

async function fixture(directory, origin, setupCode) {
  fs.mkdirSync(SHOT_DIR, { recursive: true });
  const backend = previewUnit ? new URL(origin).port : await port();
  const api = `http://127.0.0.1:${backend}`;
  const address = `127.0.0.1:${backend}`;
  const bin = process.env.HOST_BIN ?? path.join(ROOT, "target/debug/calternal-host");
  const env = {
    ...process.env,
    CALTERNAL_WORKER_BWRAP: process.env.CALTERNAL_WORKER_BWRAP ?? execFileSync("sh", ["-c", "command -v bwrap"], { encoding: "utf8" }).trim(),
    CALTERNAL_WORKER_NODE: process.env.CALTERNAL_WORKER_NODE ?? process.execPath,
    CALTERNAL_WORKER_SDK: process.env.CALTERNAL_WORKER_SDK ?? path.join(ROOT, "packages/plugin-sdk"),
  };
  const host = previewUnit ? null : spawn(bin, ["serve", directory, origin, address], { env, stdio: ["ignore", "ignore", "inherit"] });
  const stop = () => host?.kill("SIGTERM");
  process.once("exit", stop);

  const harnessDir = fs.mkdtempSync(path.join(os.tmpdir(), "calternal-test-harness-"));
  writeHarnessFixture(harnessDir);

  let server;
  let browser;
  try {
    if (previewUnit) {
      startPreview(directory, origin);
      await previewReady(origin);
    }
    else {
      await waitForHost(api, host);
      server = await preview({
        configFile: false,
        root: WEB,
        preview: {
          host: "127.0.0.1",
          port: Number(new URL(origin).port),
          strictPort: true,
          proxy: { "/v1": { target: api, ws: true } },
        },
        plugins: [
          {
            name: "spa-host-fallback",
            configurePreviewServer(srv) {
              srv.middlewares.use((req, _res, next) => {
                const pathname = (req.url || "/").split("?")[0];
                if (pathname === "/host" || pathname.startsWith("/host/")) {
                  req.url = "/index.html";
                }
                next();
              });
            },
          },
        ],
      });
    }

    browser = await chromium.launch({ headless: true });

    // --- 1. First-owner setup screen, recovery codes shown once, sign-out, passkey sign-in ---
    const a = await openContext(browser, { width: 1280, height: 800 });
    await a.page.goto(`${origin}/host`);
    await a.page.waitForSelector('h1:has-text("Set up Calternal")');
    if (previewUnit) await enterSetupCode(a.page, directory);
    else await a.page.fill('input[name="token"]', setupCode);
    await a.page.fill('input[name="name"]', "Ada");
    await a.page.click('button:has-text("Create passkey")');

    // Recovery codes step (shown once) — install plugins before entering the space so `calternal-host serve` has not opened the space lock yet.
    await a.page.waitForSelector('h1:has-text("Save your recovery codes")');
    if (previewUnit) {
      assert.equal(fs.existsSync(path.join(directory, "first-owner-code")), false, "enrollment removes the spent owner-code file before restart");
    }
    const codeCount = await a.page.locator('ul[aria-label="Recovery codes"] li').count();
    assert.ok(codeCount >= 8, `expected recovery codes, got ${codeCount}`);

    // Cookie & origin checks
    const cookie = (await a.context.cookies()).find(({ name }) => name === "__Host-calternal_session");
    assert.ok(
      cookie?.httpOnly && cookie.secure && cookie.sameSite === "Strict" && cookie.path === "/",
      "expected strict host-only browser cookie",
    );
    assert.equal(await a.page.evaluate(() => document.cookie.includes("__Host-calternal_session")), false);

    const statusBefore = await a.page.evaluate(() => fetch("/v1/auth/status").then((r) => r.json()));
    const session = statusBefore.session;
    assert.ok(session?.account && session?.space, "expected active session");

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

    // Install Chat + Test Harness filler on the real host before the browser opens the space
    if (previewUnit) service("stop");
    else await run(
      bin,
      ["install", directory, origin, session.account, session.space, path.join(ROOT, "plugins-v2/calternal-chat"), "--official"],
      { env },
    );
    await run(
      bin,
      ["install", directory, origin, session.account, session.space, harnessDir],
      { env },
    );

    if (previewUnit) {
      startPreview(directory, origin);
      await previewReady(origin);
    }

    // Confirm recovery codes and enter workspace
    await a.page.check('input[type="checkbox"]');
    await a.page.click('button:has-text("Continue")');
    await a.page.waitForSelector('[data-sidebar]');
    await a.page.waitForSelector('[data-sidebar-body] [data-cid="calternal-chat.chats"]');

    // Test sign-out (via bottom-left profile pill Account menu) and passkey sign-in UI
    await a.page.click('[data-profile]');
    await a.page.getByRole("menuitem", { name: "Sign out", exact: true }).click();
    await a.page.waitForSelector('h1:has-text("Sign in")');
    assert.equal(await a.page.locator('input[name="name"]').count(), 0, "sign-in screen must not ask for a name");
    await a.page.click('button:has-text("Sign in with a passkey")');
    await a.page.waitForSelector('[data-sidebar-body] [data-cid="calternal-chat.chats"]');
    console.log("PASS 1: passkey setup, recovery codes, sign-out, passkey sign-in, and cookie/origin checks");

    // --- 2. Create chat, send with harness chosen, rename via operation.inputs, `when` menu filtering ---
    await a.page.click('[data-canvas] button:has-text("New Chat")');
    await a.page.fill('input[name="title"]', "Release prep");
    await a.page.click('button:has-text("Create Chat")');
    await a.page.waitForSelector('[data-open-title]:has-text("Release prep")');

    // Choose harness filler from the composer slot picker (populated from s.declarations)
    await a.page.waitForSelector('[data-slot-picker="harness"]');
    await a.page.click('[data-slot-picker="harness"]');
    await a.page.getByRole("menuitem", { name: "Test Harness", exact: true }).click();
    assert.equal(
      await a.page.getAttribute('[data-slot-picker="harness"]', "data-selected-filler"),
      "test-harness",
    );

    await a.page.fill('textarea[aria-label="Write"]', "First message with harness");
    await a.page.click('button[aria-label="Send"]');
    await a.page.waitForSelector('[data-record-anchor]:has-text("First message with harness")');

    // Verify the real host invoked test-harness.runs.start via slot_selection
    let runRows = [];
    for (let attempt = 0; attempt < 20; attempt++) {
      const runsQuery = await a.page.evaluate(async (space) => {
        const res = await fetch(`/v1/spaces/${space}/call`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ call: "s.query", input: { from: "test-harness.runs" } }),
        });
        return res.json();
      }, session.space);
      runRows = runsQuery.result?.rows ?? [];
      if (runRows.length >= 1) break;
      await a.page.waitForTimeout(100);
    }
    assert.equal(runRows.length, 1, "expected slot-selected harness run record");

    // Clear harness selection for plain messages
    await a.page.click('[data-slot-picker="harness"]');
    await a.page.getByRole("menuitem", { name: "None", exact: true }).click();

    // Verify `when` hides `Unpin` while unpinned, then test Rename via operation.inputs
    await a.page.click('button[aria-label="Window menu"]');
    await a.page.getByRole("menuitem", { name: "Pin", exact: true }).waitFor();
    assert.equal(
      await a.page.getByRole("menuitem", { name: "Unpin", exact: true }).count(),
      0,
      "Unpin must be hidden by row.when",
    );
    await a.page.getByRole("menuitem", { name: "Rename", exact: true }).click();
    await a.page.fill('input[name="title"]', "Release prep v2");
    await a.page.click('button[type="submit"]:has-text("Rename")');
    await a.page.waitForSelector('[data-open-title]:has-text("Release prep v2")');

    // Toggle Pin and verify `row.when` flips: Pin hidden, Unpin shown
    await a.page.click('button[aria-label="Window menu"]');
    await a.page.getByRole("menuitem", { name: "Pin", exact: true }).click();
    await a.page.waitForTimeout(250);
    await a.page.click('button[aria-label="Window menu"]');
    await a.page.getByRole("menuitem", { name: "Unpin", exact: true }).waitFor();
    assert.equal(
      await a.page.getByRole("menuitem", { name: "Pin", exact: true }).count(),
      0,
      "Pin must be hidden by row.when after pinning",
    );
    await a.page.keyboard.press("Escape");
    console.log("PASS 2: create, send with harness slot selection, rename via operation.inputs, and row.when menu filtering");

    // --- 3. Streaming reply + Stop control surface (local fixture) ---
    await a.page.evaluate(() =>
      window.__hostLive.startStreamDemo(
        "Claude Code",
        "Checking the build logs and preparing a concise summary for the release notes.",
      ),
    );
    await a.page.waitForSelector('[data-stream-run]');
    await a.page.waitForSelector('button[aria-label="Stop"]');
    await a.page.waitForTimeout(220);
    const partialText = await a.page.locator('[data-stream-run] .body').innerText();
    assert.ok(partialText.length > 5, "expected growing reply text");
    await a.page.click('button[aria-label="Stop"]');
    await a.page.waitForSelector('[data-stream-run]', { state: "detached" });
    console.log("PASS 3: growing streamed reply and Stop control fixture");

    // --- 4. Two independent browser contexts: live append, offline reconnect from anchor, sign-out ---
    const b = await openContext(browser, { width: 1280, height: 800 });
    const { credentials } = await a.cdp.send("WebAuthn.getCredentials", { authenticatorId: a.authenticatorId });
    assert.equal(credentials.length, 1);
    await b.cdp.send("WebAuthn.addCredential", { authenticatorId: b.authenticatorId, credential: credentials[0] });

    await b.page.goto(`${origin}/host`);
    await b.page.waitForSelector('h1:has-text("Sign in")');
    await b.page.click('button:has-text("Sign in with a passkey")');
    await b.page.waitForSelector('button:has-text("Release prep v2")');
    await b.page.click('button:has-text("Release prep v2")');
    await b.page.waitForSelector('[data-record-anchor]:has-text("First message with harness")');

    // Append in A -> appears live in B
    await a.page.fill('textarea[aria-label="Write"]', "Second live message");
    await a.page.click('button[aria-label="Send"]');
    await a.page.waitForSelector('[data-record-anchor]:has-text("Second live message")');
    await b.page.waitForSelector('[data-record-anchor]:has-text("Second live message")');

    // Offline reconnect from last anchor
    await b.context.setOffline(true);
    await b.page.evaluate(() => window.__hostLive.forceReconnect());
    await b.page.waitForSelector('[data-shell][data-live="reconnecting"]');

    await a.page.fill('textarea[aria-label="Write"]', "Third after drop");
    await a.page.click('button[aria-label="Send"]');
    await a.page.waitForSelector('[data-record-anchor]:has-text("Third after drop")');

    await a.page.fill('textarea[aria-label="Write"]', "Fourth after drop");
    await a.page.click('button[aria-label="Send"]');
    await a.page.waitForSelector('[data-record-anchor]:has-text("Fourth after drop")');

    await b.context.setOffline(false);
    await b.page.waitForSelector('[data-record-anchor]:has-text("Fourth after drop")');
    const textsB = await b.page.locator("[data-record-anchor] .body").allInnerTexts();
    assert.deepEqual(textsB, [
      "First message with harness",
      "Second live message",
      "Third after drop",
      "Fourth after drop",
    ]);

    // Sign out B -> returns to sign-in; A stays signed in
    await b.page.click('[data-profile]');
    await b.page.getByRole("menuitem", { name: "Sign out", exact: true }).click();
    await b.page.waitForSelector('h1:has-text("Sign in")');
    const statusA = await a.page.evaluate(() => fetch("/v1/auth/status").then((r) => r.json()));
    assert.equal(statusA.session.account, session.account);
    console.log("PASS 4: two-context live append, anchored reconnect ordering, and sign-out revocation");

    // --- 5. Screenshots (pair-<size>-<scheme>-real.png + pair-<size>-<scheme>-mockup.png + list shots) ---
    await a.page.click('[data-sidebar-body] [data-cid="calternal-chat.chats"] .tog');
    await a.page.click('[data-canvas] button:has-text("New Chat")');
    await a.page.fill('input[name="title"]', "Weekly planning");
    await a.page.click('button:has-text("Create Chat")');
    await a.page.waitForSelector('[data-open-title]:has-text("Weekly planning")');
    await a.page.fill('textarea[aria-label="Write"]', "Outline goals for Monday and check open reviews.");
    await a.page.click('button[aria-label="Send"]');
    await a.page.waitForSelector('[data-record-anchor]:has-text("Outline goals")');

    // Desktop 1280 light — open chat (real)
    await a.page.click('[data-sidebar-body] button:has-text("Release prep v2") >> nth=0');
    await a.page.waitForSelector('[data-open-title]:has-text("Release prep v2")');
    await a.page.waitForTimeout(350);
    await a.page.screenshot({ path: path.join(SHOT_DIR, "pair-1280-light-real.png") });

    // Desktop 1280 light — list
    await a.page.click('[data-sidebar-body] [data-cid="calternal-chat.chats"] .tog');
    await a.page.waitForSelector('[data-open-title]:has-text("Chats")');
    await a.page.waitForTimeout(350);
    await a.page.screenshot({ path: path.join(SHOT_DIR, "desktop-1280-light-list.png") });

    // Desktop 1280 dark — list & open chat (real)
    await a.page.click('[data-profile]');
    await a.page.getByRole("menuitem", { name: "Dark appearance", exact: true }).click();
    await a.page.waitForTimeout(350);
    await a.page.screenshot({ path: path.join(SHOT_DIR, "desktop-1280-dark-list.png") });

    await a.page.click('[data-sidebar-body] button:has-text("Release prep v2") >> nth=0');
    await a.page.waitForSelector('[data-open-title]:has-text("Release prep v2")');
    await a.page.waitForTimeout(350);
    await a.page.screenshot({ path: path.join(SHOT_DIR, "pair-1280-dark-real.png") });

    // Phone 390 light & dark — sidebar/list (strip visible) and open chat (strip hidden!)
    const phone = await openContext(browser, { width: 390, height: 844 });
    const { credentials: latestCreds } = await b.cdp.send("WebAuthn.getCredentials", { authenticatorId: b.authenticatorId });
    await phone.cdp.send("WebAuthn.addCredential", { authenticatorId: phone.authenticatorId, credential: latestCreds[0] });
    await phone.page.goto(`${origin}/host?scheme=light`);
    await phone.page.waitForSelector('h1:has-text("Sign in")');
    await phone.page.click('button:has-text("Sign in with a passkey")');
    await phone.page.waitForSelector('[data-sidebar-body] button:has-text("Release prep v2")');
    await phone.page.waitForTimeout(350);
    assert.equal(await phone.page.locator('[data-switcher]').count(), 1, "strip visible on phone sidebar/list");
    await phone.page.screenshot({ path: path.join(SHOT_DIR, "phone-390-light-list.png") });

    await phone.page.click('[data-sidebar-body] button:has-text("Release prep v2") >> nth=0');
    await phone.page.waitForSelector('[data-open-title]:has-text("Release prep v2")');
    await phone.page.mouse.move(0, 0);
    await phone.page.waitForTimeout(400);
    assert.equal(
      await phone.page.locator('[data-switcher]').count(),
      0,
      "bottom strip must be hidden on phone while a chat is open",
    );
    await phone.page.screenshot({ path: path.join(SHOT_DIR, "pair-390-light-real.png") });

    // Phone 390 dark — list & open chat
    await phone.page.goto(`${origin}/host?scheme=dark`);
    await phone.page.waitForSelector('[data-sidebar-body] button:has-text("Release prep v2")');
    await phone.page.waitForTimeout(350);
    await phone.page.screenshot({ path: path.join(SHOT_DIR, "phone-390-dark-list.png") });

    await phone.page.click('[data-sidebar-body] button:has-text("Release prep v2") >> nth=0');
    await phone.page.waitForSelector('[data-open-title]:has-text("Release prep v2")');
    await phone.page.mouse.move(0, 0);
    await phone.page.waitForTimeout(400);
    assert.equal(await phone.page.locator('[data-switcher]').count(), 0);
    await phone.page.screenshot({ path: path.join(SHOT_DIR, "pair-390-dark-real.png") });

    if (previewUnit) {
      console.log(`PASS 5: real preview screenshots written to ${SHOT_DIR}`);
      return;
    }

    // Capture matching mockup pairs from http://10.69.69.237:4313/ (Chat app, one chat open)
    const mockupUrl = process.env.MOCKUP_URL || "http://10.69.69.237:4313";
    for (const [label, vp, qs] of [
      ["1280-light", { width: 1280, height: 800 }, "/?app=chat&scheme=light"],
      ["1280-dark", { width: 1280, height: 800 }, "/?app=chat&scheme=dark"],
      ["390-light", { width: 390, height: 844 }, "/?app=chat&pushed=1&scheme=light"],
      ["390-dark", { width: 390, height: 844 }, "/?app=chat&pushed=1&scheme=dark"],
    ]) {
      const mctx = await browser.newContext({ viewport: vp });
      const mp = await mctx.newPage();
      try {
        await mp.goto(`${mockupUrl}${qs}`, { waitUntil: "networkidle", timeout: 15_000 });
        await mp.waitForTimeout(400);
        await mp.screenshot({ path: path.join(SHOT_DIR, `pair-${label}-mockup.png`) });
      } finally {
        await mctx.close();
      }
    }

    console.log(`PASS 5: all real + mockup side-by-side screenshots written to ${SHOT_DIR}`);
  } finally {
    process.removeListener("exit", stop);
    await browser?.close();
    server?.httpServer?.close();
    fs.rmSync(harnessDir, { recursive: true, force: true });
    if (host && host.exitCode === null && host.signalCode === null) {
      const exited = once(host, "exit");
      host.kill("SIGTERM");
      await exited;
    }
  }
}

if (process.argv[2] === "--fixture") {
  const setupCode = previewUnit ? null : await new Promise((resolve, reject) => {
    let value = "";
    process.stdin
      .setEncoding("utf8")
      .on("data", (chunk) => {
        value += chunk;
      })
      .on("end", () => resolve(value))
      .on("error", reject);
  });
  await fixture(process.argv[3], process.argv[4], setupCode);
} else {
  const origin = `http://localhost:${await port()}`;
  await run("cargo", ["run", "--quiet", "-p", "calternal-host", "--example", "browser-chat-fixture", "--", origin], {
    cwd: ROOT,
  });
}
