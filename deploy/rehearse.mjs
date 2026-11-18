// Dev-only proof through the deployed web origin. Uses an ephemeral software passkey.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { request as httpsRequest } from "node:https";
import { createRequire } from "node:module";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
const { chromium } = createRequire(new URL("../apps/web/package.json", import.meta.url))("playwright");

process.umask(0o077);
const [phase, origin, root, target, tag, proof, runArgument] = process.argv.slice(2);
const unit = process.env.CALTERNAL_REHEARSAL_UNIT ?? "calternal-ai-rehearsal";
const settingUp = ["setup", "upgrade-setup"].includes(phase);
assert.ok(["setup", "fixture", "verify", "restart", "real", "upgrade-setup", "upgrade"].includes(phase)
  && origin && root);
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
    ...(settingUp ? {} : { storageState: session }),
  });
  const page = await context.newPage();
  const shell = settingUp
    ? await context.request.get(origin)
    : await page.goto(origin, { waitUntil: "domcontentloaded" });
  assert.equal(shell.status(), 200);
  assert.match(shell.headers()["content-security-policy"], /script-src 'self'/);
  if (settingUp) {
    // Use the deployed origin for WebAuthn while keeping the app's first-owner
    // navigation out of the fixture's credential ceremony.
    const ceremonyPage = new URL("/__rehearsal-passkey", origin).href;
    await page.route(ceremonyPage, route => route.fulfill({
      contentType: "text/html",
      body: "<!doctype html><title>Rehearsal passkey</title>",
    }));
    await page.goto(ceremonyPage);
  }
  const api = async (path, data) => {
    const response = await context.request.fetch(`${origin}/api/v1/${path}`, {
      method: data === undefined ? "GET" : "POST", data,
      headers: { origin, "content-type": "application/json" },
    });
    assert.ok(response.ok(), `${path}: HTTP ${response.status()}`);
    return response.json();
  };
  const assertSignedIn = async () => {
    const { setup, signed_in: signedIn } = await api("auth/status");
    assert.equal(setup, false);
    assert.equal(signedIn, true);
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

  if (settingUp) {
    if (phase === "setup") {
      for (let index = 0; index < 20; index++) {
        assert.equal(await anonymousSetup("127.0.0.10"), 400);
      }
      assert.equal(await anonymousSetup("127.0.0.10"), 429);
      assert.equal(await anonymousSetup("127.0.0.11"), 400);
      console.log("Trusted proxy separates source buckets and overwrites spoofed forwarding headers.");
    }

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
    await assertSignedIn();
    if (phase === "upgrade-setup") {
      // Start the measured browser visit as an owner who finished onboarding.
      assert.deepEqual(await op("onboarding.finish"), { finished: true });
    }
    await context.storageState({ path: session });
    console.log("First owner registered a resident software passkey at the configured HTTPS origin.");

  }
  if (phase === "upgrade") {
    const run = Number(runArgument);
    const driver = process.env.CALTERNAL_REHEARSAL_DRIVER;
    assert.ok(target && tag && proof && driver && Number.isInteger(run) && run >= 1 && run <= 10,
      "upgrade needs a disposable target, image tag, proof, deployment driver and run number");
    const deploy = join(driver, "deploy.sh");
    const rollback = join(driver, "rollback.sh");
    const imageId = () => execFileSync("podman", [
      "container", "inspect", "--format", "{{.Image}}", unit,
    ], { encoding: "utf8" }).trim().replace(/^sha256:/, "");
    // The driver moves this pointer after stopping the old service and before
    // starting the new one. Classify only requests sent and acknowledged on
    // the same side of that move.
    const serving = () => readFileSync(join(root, "releases/current/image.id"), "utf8").trim();
    const previous = imageId();
    const evidence = JSON.parse(readFileSync(proof, "utf8"));
    assert.equal(previous, evidence.previous);
    await page.locator("html[data-ready]").waitFor({ timeout: 15_000 });
    await assertSignedIn();
    await wait("enrolled machine online", async () => (await op("machines.onboarding")).ready);
    const space = (await op("spaces.list")).spaces.find(s => s.personal && s.role === "owner").id;
    const deployArgs = ["--target", target, "--image", tag, "--compatibility-proof"];
    const wrongProof = join(root, "wrong-proof.json");
    writeFileSync(wrongProof, JSON.stringify({ ...evidence, candidate: evidence.previous }));
    const beforePid = execFileSync("podman", [
      "container", "inspect", "--format", "{{.State.Pid}}", unit,
    ], { encoding: "utf8" });
    assert.throws(() => execFileSync(deploy, [...deployArgs, wrongProof],
      { stdio: ["ignore", "pipe", "pipe"] }), error => {
      assert.match(String(error.stderr), /preflight refused/i);
      return true;
    });
    assert.equal(execFileSync("podman", ["container", "inspect", "--format", "{{.State.Pid}}", unit],
      { encoding: "utf8" }), beforePid);
    await assertSignedIn();
    console.log("Invalid proof was refused while the original container and owner session stayed up.");

    assert.equal(imageId(), previous);
    execFileSync("podman", ["tag", evidence.candidate, `localhost/${unit}:${tag}`]);
    await wait("old machine online", async () => (await op("machines.onboarding")).ready);
    const samples = [];
    let deployResult;
    let deployError;
    const schedule = () => ({
      operation: "notes.create",
      input: { space_id: space, title: `Upgrade ${run} ${crypto.randomUUID()}`, markdown: "Acknowledged write probe" },
      request_id: crypto.randomUUID(),
    });
    let pending = schedule();
    const write = async () => {
      const startedOn = serving();
      let status;
      let note;
      try {
        const response = await context.request.post(`${origin}/api/v1/operations`, {
          data: pending, headers: { origin }, timeout: 1000,
        });
        status = response.status();
        if (response.ok()) {
          note = (await response.json()).data.note;
        }
      } catch {
        status = 0;
      }
      const at = performance.now();
      samples.push({ at, status, note, image: startedOn === serving() ? startedOn : null });
      if (note) pending = schedule();
    };
    await write();
    assert.ok(samples[0].note);
    const banner = page.getByRole("status").filter({ hasText: "Reconnecting…" });
    // Start watching before the switch. The request probe also works when the
    // app doesn't visibly display the banner during a short handoff.
    const bannerSeen = banner.waitFor({ state: "visible", timeout: 0 })
      .then(() => true, () => false);
    const child = spawn(deploy, [...deployArgs, proof], {
      stdio: "inherit", detached: true,
    });
    const done = new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code, signal) => resolve({ code, signal }));
    });
    void done.then(value => { deployResult = value; }, error => { deployError = error; });
    const started = performance.now();
    try {
      while (!deployResult || !samples.at(-1).note) {
        assert.ok(performance.now() - started < 120_000, "deploy and first new write timed out");
        await sleep(40);
        await write();
        if (deployError) throw deployError;
        if (deployResult) assert.equal(deployResult.code, 0, "deploy failed");
      }
    } finally {
      if (!deployResult && child.pid) {
        // This private process group includes the driver and the lab SSH shim.
        try {
          process.kill(-child.pid, "SIGTERM");
        } catch (error) {
          if (error.code !== "ESRCH") throw error;
        }
        await done;
      }
    }

    assert.equal(imageId(), evidence.candidate);
    const old = samples.findLast(sample => sample.note && sample.image === previous);
    const next = samples.find(sample => sample.note && sample.image === evidence.candidate);
    assert.ok(old && next && next.at > old.at, "missing acknowledgements around the write gap");
    assert.ok(samples.some(sample => !sample.note && sample.at > old.at && sample.at < next.at),
      "no write failure was sampled during the stop");
    assert.equal((await op("notes.get", { id: old.note.id })).note.id, old.note.id);
    assert.equal((await op("notes.get", { id: next.note.id })).note.id, next.note.id);
    await assertSignedIn();
    console.log(JSON.stringify({
      run, gap_ms: Number((next.at - old.at).toFixed(1)),
      lost_samples: samples.filter(sample => !sample.note).length,
      failure_statuses: [...new Set(samples.filter(sample => !sample.note).map(sample => sample.status))],
      reconnect_banner: await Promise.race([bannerSeen, sleep(600).then(() => false)]),
    }));
    // The handoff above keeps the tab open. The untimed rollback check needs
    // only the session cookie and the acknowledged note.
    await page.close();
    execFileSync(rollback, ["--target", target], { stdio: "inherit" });
    assert.equal(imageId(), previous);
    assert.equal((await op("notes.get", { id: next.note.id })).note.id, next.note.id);
  } else if (phase === "upgrade-setup") {
    // This phase only initializes the disposable owner's browser session.
  } else if (["setup", "fixture", "real"].includes(phase)) {
    await wait("enrolled machine online", async () =>
      (await op("machines.onboarding")).ready);

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
    await assertSignedIn();
    const saved = JSON.parse(readFileSync(marker, "utf8"));
    assert.equal((await job(saved.id)).state, "done");
    const output = join(root, "userdata-ssd/spaces", saved.space, "AI/Chats", `${saved.title}.md`);
    assert.match(readFileSync(output, "utf8"), /Fixture turn before upgrade/);
    console.log("Owner session, completed job and Markdown survived the image switch or service restart.");

    if (phase === "restart") {
      await wait("enrolled machine online", async () =>
        (await op("machines.onboarding")).ready);
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
