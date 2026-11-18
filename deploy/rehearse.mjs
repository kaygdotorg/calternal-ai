// Dev-only proof through the deployed web origin. Uses an ephemeral software passkey.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { request as httpsRequest } from "node:https";
import { createRequire } from "node:module";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
const { chromium } = createRequire(new URL("../apps/web/package.json", import.meta.url))("playwright");

process.umask(0o077);
const unit = "calternal-ai-rehearsal";
const [phase, origin, root, machineUrl] = process.argv.slice(2);
assert.ok(["setup", "fixture", "verify", "restart", "real"].includes(phase) && origin && root && machineUrl);
const session = join(root, "rehearsal-session.json");
const cli = join(root, "releases/current/calternal");
const machine = join(root, "machine");
const sandbox = join(machine, "sandboxes");
const marker = join(root, "rehearsal-thread.json");

function systemctl(...args) {
  execFileSync("systemctl", ["--user", ...args], { stdio: "inherit" });
}

async function wait(label, check, timeout = 45_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await check()) return;
    await sleep(500);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

function anonymousSetup(source) {
  return new Promise((accept, reject) => {
    const target = new URL("/api/v1/auth/setup/start", origin);
    const payload = JSON.stringify({ token: crypto.randomUUID(), name: "Rate-limit probe" });
    const req = httpsRequest(target, {
      method: "POST", rejectUnauthorized: false, localAddress: source, family: 4,
      headers: {
        Origin: origin, "Content-Type": "application/json",
        "Content-Length": Buffer.byteLength(payload),
        "X-Forwarded-For": `198.51.100.${Math.floor(Math.random() * 253) + 1}`,
      },
    }, res => {
      res.resume();
      res.on("end", () => accept(res.statusCode));
    });
    req.on("error", reject);
    req.end(payload);
  });
}

function containers() {
  return execFileSync("podman", [
    "ps", "-aq", "--filter", `label=org.calternal.sandbox-root=${sandbox}`,
  ], { encoding: "utf8" }).trim().split("\n").filter(Boolean);
}

const browser = await chromium.launch();
try {
  const context = await browser.newContext({
    ignoreHTTPSErrors: true,
    ...(phase === "setup" ? {} : { storageState: session }),
  });
  const page = await context.newPage();
  const shell = await page.goto(origin, { waitUntil: "domcontentloaded" });
  assert.equal(shell.status(), 200);
  assert.match(shell.headers()["content-security-policy"], /script-src 'self'/);
  const api = async (path, data) => {
    const response = await context.request.fetch(`${origin}/api/v1/${path}`, {
      method: data === undefined ? "GET" : "POST", data,
      headers: { origin, "content-type": "application/json" },
    });
    assert.ok(response.ok(), `${path}: HTTP ${response.status()}`);
    return response.json();
  };
  const op = async (operation, input = {}) => (await api("operations", {
    operation, input, request_id: crypto.randomUUID(),
  })).data;
  const job = async id => (await op("jobs.get", { job_id: id })).job;
  const completed = async id => {
    const current = await job(id);
    if (["failed", "cancelled", "interrupted", "outcome_unknown", "retry_waiting"].includes(current.state)) {
      throw new Error(`Job ${current.state} (${current.checkpoint?.error?.code ?? "no error code"})`);
    }
    return current.state === "done";
  };

  if (phase === "setup") {
    for (let index = 0; index < 20; index++) {
      assert.equal(await anonymousSetup("127.0.0.10"), 400);
    }
    assert.equal(await anonymousSetup("127.0.0.10"), 429);
    assert.equal(await anonymousSetup("127.0.0.11"), 400);
    console.log("Trusted proxy separates source buckets and overwrites spoofed forwarding headers.");

    const tokenFile = join(root, "appdata/setup.token");
    assert.equal(statSync(tokenFile).mode & 0o777, 0o600);
    const cdp = await context.newCDPSession(page);
    await cdp.send("WebAuthn.enable");
    await cdp.send("WebAuthn.addVirtualAuthenticator", { options: {
      protocol: "ctap2", transport: "internal", hasResidentKey: true,
      hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true,
    }});
    const start = await api("auth/setup/start", {
      token: readFileSync(tokenFile, "utf8").trim(), name: "Deployment rehearsal owner",
    });
    assert.equal(start.options.publicKey.rp.id, new URL(origin).hostname);
    const credential = await page.evaluate(async options =>
      (await navigator.credentials.create({
        publicKey: PublicKeyCredential.parseCreationOptionsFromJSON(options),
      })).toJSON(), start.options.publicKey);
    await api("auth/setup/finish", { ceremony: start.ceremony, credential, device: "Dev rehearsal" });
    assert.deepEqual(await api("auth/status"), { setup: false, signed_in: true });
    await context.storageState({ path: session });
    console.log("First owner registered a resident software passkey at the configured HTTPS origin.");

    const enrollment = await api("machines/codes", {});
    execFileSync(cli, ["machine", "enroll", "--server", machineUrl, "--dir", machine,
      "--name", "Deployment rehearsal", "--code", "-"], {
      input: enrollment.code, env: { ...process.env, NO_PROXY: new URL(machineUrl).hostname }, stdio: ["pipe", "ignore", "inherit"],
    });
    systemctl("start", `${unit}-machine.service`);
  }
  if (["setup", "fixture", "real"].includes(phase)) {
    await wait("enrolled machine online", async () =>
      (await api("machines")).machines.some(m => m.state === "online"));

    const space = (await op("spaces.list")).spaces.find(s => s.personal && s.role === "owner").id;
    const title = phase === "real" ? "Real provider rehearsal" : `Deployment rehearsal ${crypto.randomUUID().slice(0, 8)}`;
    const message = phase === "real" ? "Reply with the single word READY." : "Fixture turn before upgrade";
    const thread = (await op("threads.create", { space_id: space, title })).thread.id;
    const id = (await op("threads.send", { id: thread, message })).job.id;
    await wait("turn completion", () => completed(id), phase === "real" ? 120_000 : 45_000);
    const finished = await job(id);
    assert.ok(finished.checkpoint.session_id);
    if (phase === "real") {
      console.log(`Real provider completed the turn in /threads/${thread}.`);
      process.exitCode = 0;
    } else {
      const rootfs = await context.request.get(`${origin}/threads/${thread}`);
      assert.equal(rootfs.status(), 200);
      const output = join(root, "userdata-ssd/spaces", space, "AI/Chats", `${title}.md`);
      assert.match(readFileSync(output, "utf8"), /Fixture turn before upgrade/);
      writeFileSync(marker, JSON.stringify({ id, thread, space, title }));
      await wait("fixture container cleanup", () => containers().length === 0);
      console.log("Enrolled machine completed a fixture turn, saved its Markdown and released its container.");
    }
  } else {
    assert.deepEqual(await api("auth/status"), { setup: false, signed_in: true });
    const saved = JSON.parse(readFileSync(marker, "utf8"));
    assert.equal((await job(saved.id)).state, "done");
    const output = join(root, "userdata-ssd/spaces", saved.space, "AI/Chats", `${saved.title}.md`);
    assert.match(readFileSync(output, "utf8"), /Fixture turn before upgrade/);
    console.log("Owner session, completed job and Markdown survived the image switch or service restart.");

    if (phase === "restart") {
      await wait("enrolled machine online", async () =>
        (await api("machines")).machines.some(m => m.state === "online"));
      const id = (await op("threads.send", {
        id: saved.thread, message: "Fixture turn for restart hang",
      })).job.id;
      await wait("active sandbox and saved checkpoint", async () =>
        containers().length === 1 && Boolean((await job(id)).checkpoint.session_id));
      const [cid] = containers();
      const [inspect] = JSON.parse(execFileSync("podman", ["inspect", cid], { encoding: "utf8" }));
      assert.equal(inspect.HostConfig.Memory, 1_073_741_824);
      assert.equal(inspect.HostConfig.PidsLimit, 256);
      assert.equal(inspect.HostConfig.CpuQuota / inspect.HostConfig.CpuPeriod, 1);
      assert.ok(!inspect.HostConfig.Privileged);
      const activeBinary = execFileSync("realpath", [cli], { encoding: "utf8" }).trim();
      const expected = new Map([
        ["/home/agent", sandbox], ["/work", sandbox], ["/nix/store", "/nix/store"],
        ["/usr/local/bin/calternal", activeBinary],
      ]);
      for (const mount of inspect.Mounts) {
        const prefix = expected.get(mount.Destination);
        assert.ok(prefix && (mount.Source === prefix || mount.Source.startsWith(`${prefix}/`)),
          `Unexpected sandbox mount: ${mount.Destination}`);
        if (mount.Destination === "/nix/store" || mount.Destination === "/usr/local/bin/calternal") {
          assert.equal(mount.RW, false);
        }
      }
      assert.notEqual(inspect.HostConfig.NetworkMode, "host");
      execFileSync("podman", ["exec", cid, "/bin/sh", "-c",
        "test ! -e /data/appdata && test ! -e /srv/calternal-app/data && test ! -e /run/podman/podman.sock"]);
      console.log("Running sandbox had the configured CPU/memory/PID bounds and no server data or Podman socket mounts.");

      systemctl("stop", `${unit}-machine.service`);
      systemctl("restart", `${unit}.service`);
      systemctl("start", `${unit}-machine.service`);
      await wait("server health", async () => {
        const res = await context.request.get(`${origin}/api/v1/auth/status`);
        return res.ok();
      });
      await wait("interrupted checkpoint", async () => (await job(id)).state === "interrupted");
      const interrupted = await job(id);
      assert.ok(interrupted.checkpoint.session_id);
      await op("agent.resume", { job_id: id });
      await wait("resumed fixture completion", () => completed(id));
      assert.equal((await job(id)).checkpoint.session_id, interrupted.checkpoint.session_id);
      await wait("all lane sandboxes removed", () => containers().length === 0);
      console.log("Restart fenced the running turn. Explicit Resume used the saved session and completed without leaks.");
    }
  }
} finally {
  await browser.close();
}
