#!/usr/bin/env node
/**
 * End-to-end Playwright test for slices 4 and 5 (declaration-driven host client + live window).
 *
 * Tests:
 * 1. Sign in, create a chat, send a message, rename the chat (all from declarations).
 * 2. Two independent browser contexts on one chat:
 *    - An append in context A appears live in context B.
 *    - Context B drops its WebSocket, context A appends two more messages, and
 *      context B reconnects from its last anchor with exact order and zero duplicates.
 * 3. Captures desktop (1280x800) and phone (390x844) screenshots in light and dark
 *    mode into `HOST_SHOT_DIR` (outside the repository, for uploading to the Forgejo issue).
 */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB_DIR = path.resolve(__dirname, "..");
const MOCK_PORT = Number(process.env.HOST_MOCK_PORT || 4391);
const WEB_PORT = Number(process.env.HOST_WEB_PORT || 4392);
const SHOT_DIR = process.env.HOST_SHOT_DIR || "/tmp/browser-chat-shots";

function waitForLine(proc, regex, label) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`Timed out waiting for ${label}`)), 30_000);
    const onData = (buf) => {
      const s = buf.toString();
      if (regex.test(s)) {
        clearTimeout(timer);
        resolve(s);
      }
    };
    proc.stdout.on("data", onData);
    proc.stderr.on("data", onData);
    proc.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`${label} exited early with code ${code}`));
    });
  });
}

async function main() {
  fs.mkdirSync(SHOT_DIR, { recursive: true });

  const mock = spawn("node", [path.join(WEB_DIR, "scripts/host-mock.mjs")], {
    cwd: WEB_DIR,
    env: { ...process.env, HOST_MOCK_PORT: String(MOCK_PORT), HOST_MOCK_SEED: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });

  const web = spawn(
    "npx",
    ["vite", "preview", "--host", "127.0.0.1", "--port", String(WEB_PORT), "--strictPort"],
    {
      cwd: WEB_DIR,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );

  let browser;
  try {
    await waitForLine(mock, /host-mock listening/, "host-mock");
    await waitForLine(web, new RegExp(`:${WEB_PORT}`), "vite preview");

    const api = `http://127.0.0.1:${MOCK_PORT}`;
    const baseUrl = `http://127.0.0.1:${WEB_PORT}/host?api=${encodeURIComponent(api)}`;

    browser = await chromium.launch({ headless: true });

    // --- Test 1: Sign in, make a chat, send a message, rename the chat ---
    const ctxA = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const pageA = await ctxA.newPage();
    await pageA.goto(baseUrl);

    await pageA.fill('input[name="name"]', "Ada");
    await pageA.click('button[type="submit"]');

    // Sidebar drawn from sidebars/chat.md
    await pageA.waitForSelector('nav[aria-label="Chat"]');
    await pageA.waitForSelector('button:has-text("Pinned")');
    await pageA.waitForSelector('button:has-text("Chats")');
    await pageA.waitForSelector('button:has-text("Archived")');

    // Create a new chat via the "+" button derived from `add: title`
    await pageA.click('button[aria-label="New Chat"] >> nth=0');
    await pageA.fill('input[name="title"]', "Release prep");
    await pageA.click('button:has-text("Create Chat")');

    await pageA.waitForSelector('[data-open-title]:has-text("Release prep")');

    // Send a message via the composer slot
    await pageA.fill('textarea[aria-label="Write"]', "First message from Ada");
    await pageA.click('button[aria-label="Send"]');
    await pageA.waitForSelector('[data-record-anchor]:has-text("First message from Ada")');

    // Rename the chat via the item actions menu generated from `rename: code, menu`
    await pageA.click('button[aria-label="Item actions"]');
    await pageA.click('button:has-text("Rename")');
    await pageA.fill('input[name="title"]', "Release prep v2");
    await pageA.click('button[type="submit"]:has-text("Rename")');
    await pageA.waitForSelector('[data-open-title]:has-text("Release prep v2")');
    console.log("PASS test 1: sign in, create, send, rename");

    // --- Test 2: Two contexts live sync + reconnect from last anchor ---
    const ctxB = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const pageB = await ctxB.newPage();
    await pageB.goto(baseUrl);
    await pageB.fill('input[name="name"]', "Ada");
    await pageB.click('button[type="submit"]');
    await pageB.waitForSelector('button:has-text("Release prep v2")');
    await pageB.click('button:has-text("Release prep v2")');
    await pageB.waitForSelector('[data-record-anchor]:has-text("First message from Ada")');

    // Append in A -> appears live in B
    await pageA.fill('textarea[aria-label="Write"]', "Second live message");
    await pageA.click('button[aria-label="Send"]');
    await pageB.waitForSelector('[data-record-anchor]:has-text("Second live message")');

    // Force-drop B's WebSocket, send two ordered messages from A, verify B catches up in order
    await pageB.evaluate(() => window.__hostLive.forceReconnect());
    await pageA.fill('textarea[aria-label="Write"]', "Third after drop");
    await pageA.click('button[aria-label="Send"]');
    await pageA.waitForSelector('[data-record-anchor]:has-text("Third after drop")');

    await pageA.fill('textarea[aria-label="Write"]', "Fourth after drop");
    await pageA.click('button[aria-label="Send"]');
    await pageA.waitForSelector('[data-record-anchor]:has-text("Fourth after drop")');

    await pageB.waitForSelector('[data-record-anchor]:has-text("Fourth after drop")');
    const textsB = await pageB.locator("[data-record-anchor] .body").allInnerTexts();
    assert.deepEqual(textsB, [
      "First message from Ada",
      "Second live message",
      "Third after drop",
      "Fourth after drop",
    ]);
    console.log("PASS test 2: two-context live append and reconnect-from-anchor ordering");

    // --- Screenshots (desktop 1280 & phone 390, light & dark) ---
    await pageA.click('button:has-text("Dependency audit")');
    await pageA.waitForSelector('[data-record-anchor]:has-text("Morning!")');
    await pageA.screenshot({ path: path.join(SHOT_DIR, "desktop-1280-light.png") });

    await pageA.click('button[aria-label="Switch to dark mode"]');
    await pageA.waitForTimeout(250);
    await pageA.screenshot({ path: path.join(SHOT_DIR, "desktop-1280-dark.png") });

    const ctxPhone = await browser.newContext({ viewport: { width: 390, height: 844 } });
    const pagePhone = await ctxPhone.newPage();
    await pagePhone.goto(`${baseUrl}&scheme=light`);
    await pagePhone.fill('input[name="name"]', "Ada");
    await pagePhone.click('button[type="submit"]');
    await pagePhone.waitForSelector('button:has-text("Dependency audit")');
    await pagePhone.click('button:has-text("Dependency audit")');
    await pagePhone.waitForSelector('[data-record-anchor]:has-text("Morning!")');
    await pagePhone.screenshot({ path: path.join(SHOT_DIR, "phone-390-light.png") });

    await pagePhone.click('button[aria-label="Switch to dark mode"]');
    await pagePhone.waitForTimeout(250);
    await pagePhone.screenshot({ path: path.join(SHOT_DIR, "phone-390-dark.png") });

    await ctxA.close();
    await ctxB.close();
    await ctxPhone.close();
    console.log(`PASS screenshots written to ${SHOT_DIR}`);
  } finally {
    await browser?.close();
    mock.kill("SIGTERM");
    web.kill("SIGTERM");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
