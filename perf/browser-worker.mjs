// Warm browser → server → enrolled machine → plugin → browser gate.
import { spawn, spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const [serverBinary, pluginRoot, chromium, reportPath] = process.argv.slice(2);
if (process.env.CALTERNAL_PERF_HELD !== "1" || process.argv.length !== 6) {
  throw new Error("Usage under perf-lock.sh: node browser-worker.mjs SERVER PLUGINS CHROMIUM REPORT");
}

const metric = "plugin.warm_browser_worker";
const budgets = JSON.parse(readFileSync(new URL("./budgets.json", import.meta.url), "utf8"));
const [limit, ...duplicates] = budgets.entries.filter(entry => entry.metric === metric);
if (duplicates.length || limit?.percentile !== "p95" || limit.unit !== "ms"
    || !Number.isFinite(limit.budget) || limit.budget <= 0) {
  throw new Error("Expected one positive p95 budget in milliseconds for " + metric);
}

const binary = resolve(serverBinary);
const podman = process.env.CALTERNAL_PERF_PODMAN || "podman";
const runner = process.env.CALTERNAL_PERF_WORKER_RUNNER;
const root = mkdtempSync("/tmp/calternal-browser-warm-");
const machine = join(root, "machine");
const children = [];
const logs = new Map();
const storeArgs = [
  "--appdata", join(root, "appdata"),
  "--userdata-ssd", join(root, "ssd"),
  "--userdata-hdd", join(root, "hdd"),
  "--installation-id", randomUUID(),
];

function start(name, file, args) {
  const child = spawn(file, args, { stdio: ["ignore", "pipe", "pipe"] });
  children.push(child);
  logs.set(name, "");
  const log = chunk => logs.set(name, (logs.get(name) + chunk).slice(-4000));
  child.on("error", error => log(error.message));
  child.stdout.on("data", log);
  child.stderr.on("data", log);
}

async function waitFor(description, read) {
  const deadline = performance.now() + 22_500;
  do {
    const ready = await read();
    if (ready) return ready;
    await sleep(50);
  } while (performance.now() < deadline);
  throw new Error("Timed out waiting for " + description);
}

function cli(args, env = {}) {
  const result = spawnSync(binary, [...args, "--json"], {
    encoding: "utf8", env: { ...process.env, ...env }, timeout: 10_000,
  });
  const { data, error } = JSON.parse(result.stdout || "{}");
  if (result.status !== 0 || !data) {
    const detail = args[0] === "plugin" ? ` (${error?.message || "no details"})` : "";
    throw new Error(`Fixture ${args.slice(0, 2).join(" ")} failed: ${error?.code || result.error?.code || "unknown"}${detail}`);
  }
  return data;
}

async function measure(token, operation, input, expected) {
  const samples = [];
  for (let sample = 0; sample < 110; sample++) {
    const start = performance.now();
    const response = await fetch("/api/v1/operations", {
      method: "POST",
      headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ request_id: crypto.randomUUID(), operation, input }),
      signal: AbortSignal.timeout(6000),
    });
    const result = await response.json();
    const elapsed = performance.now() - start;
    if (!response.ok || result.data?.view?.source !== expected) {
      throw new Error("Expected the fixture's generated view: " + response.status);
    }
    if (sample >= 10) samples.push(elapsed);
  }
  return { browser: navigator.userAgent, samples };
}

async function measureSettingsFeed(base, token, address, invoke) {
  const controller = new AbortController();
  const url = new URL("/api/v1/live", base);
  url.searchParams.set("subscriptions", JSON.stringify([{ space_id: address.space_id, bookmark: null }]));
  const response = await fetch(url, {
    headers: { authorization: "Bearer " + token, origin: new URL(base).origin },
    signal: AbortSignal.any([controller.signal, AbortSignal.timeout(30_000)]),
  });
  if (!response.ok) throw new Error("Setting feed unavailable: " + response.status);
  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  async function next() {
    while (true) {
      const index = buffer.indexOf("\n\n");
      if (index >= 0) {
        const data = buffer.slice(0, index).split("\n").find(line => line.startsWith("data: "));
        buffer = buffer.slice(index + 2);
        if (data) return JSON.parse(data.slice(6));
      } else {
        const { done, value } = await reader.read();
        if (done) throw new Error("Setting feed ended before invalidation");
        buffer += value;
      }
    }
  }
  try {
    if ((await next()).type !== "ready") throw new Error("Expected the setting feed ready frame");
    const samples = [];
    let previous = null;
    for (let i = 0; i < 35; i++) {
      const value = "Timing label " + i;
      const started = performance.now();
      const write = invoke("plugins.settings.set", { ...address, base: previous, value });
      while (true) {
        const frame = await next();
        if (frame.entry?.kind === "plugins.settings.set"
          && frame.entry.data.plugin_id === address.plugin_id && frame.entry.data.key === address.key) break;
      }
      const elapsed = performance.now() - started;
      if ((await write).data?.conflicted !== false) throw new Error("Setting write did not commit");
      if (i >= 5) samples.push(elapsed);
      previous = value;
    }
    return samples;
  } finally {
    controller.abort();
  }
}

let socket;
let nextCommand = 0;
function command(method, params = {}) {
  const id = ++nextCommand;
  return new Promise((resolve, reject) => {
    function dispose() {
      clearTimeout(timeout);
      socket.removeEventListener("message", onMessage);
    }
    function onMessage(event) {
      const reply = JSON.parse(event.data);
      if (reply.id !== id) return;
      dispose();
      if (reply.error) reject(new Error(method + " failed"));
      else resolve(reply.result);
    }
    const timeout = setTimeout(() => {
      dispose();
      reject(new Error(method + " timed out"));
    }, 30_000);
    socket.addEventListener("message", onMessage);
    socket.send(JSON.stringify({ id, method, params }));
  });
}

async function evaluate(expression) {
  const evaluation = await command("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (evaluation.exceptionDetails) throw new Error("Fixture evaluation failed: " + evaluation.exceptionDetails.text);
  return evaluation.result.value;
}

try {
  const web = join(root, "web");
  mkdirSync(web);
  writeFileSync(join(web, "index.html"), "<!DOCTYPE html><title>Warm operation fixture</title>");
  start("server", binary, [
    "serve", "--create", "--development", "--port", "0", ...storeArgs,
    "--web-dir", web, "--plugin-root", resolve(pluginRoot),
  ]);
  const startupFile = join(root, "appdata/startup.json");
  const startup = await waitFor("server startup", () =>
    existsSync(startupFile) && JSON.parse(readFileSync(startupFile, "utf8")));
  const token = readFileSync(startup.token_files.owner, "utf8").trim();
  async function invoke(operation, input) {
    const response = await fetch(startup.base_url + "/api/v1/operations", {
      method: "POST",
      headers: {
        authorization: "Bearer " + token, "content-type": "application/json",
        origin: new URL(startup.base_url).origin,
      },
      body: JSON.stringify({ request_id: randomUUID(), operation, input }),
      signal: AbortSignal.timeout(6000),
    });
    return response.json();
  }

  const { data } = await invoke("spaces.create", { name: "Browser timing fixture" });
  const space = data?.space;
  if (!space?.owner || !space.id) throw new Error("Fixture space creation failed");
  cli(["machines", "pool-grant", space.owner, ...storeArgs]);
  const { code } = cli(["machines", "code", "--pool", ...storeArgs]);
  cli(["machine", "enroll", "--server", startup.base_url, "--dir", machine], { CALTERNAL_ENROLL_CODE: code });
  start("machine", runner || binary, [
    ...(runner ? [] : ["machine"]), "run", "--dir", machine, "--backend", "podman",
    "--max-sandboxes", "8", "--podman", podman,
  ]);

  const profile = join(root, "browser");
  mkdirSync(profile);
  start("chrome", resolve(chromium), [
    "--no-sandbox", "--disable-dev-shm-usage", "--remote-debugging-port=0",
    "--user-data-dir=" + profile, "about:blank",
  ]);
  const portFile = join(profile, "DevToolsActivePort");
  const port = await waitFor("browser debug port", () =>
    existsSync(portFile) && readFileSync(portFile, "utf8").split("\n")[0]);
  const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`, {
    signal: AbortSignal.timeout(5000),
  })).json();
  socket = new WebSocket(tabs[0].webSocketDebuggerUrl);
  await once(socket, "open", { signal: AbortSignal.timeout(5000) });
  await command("Page.navigate", { url: startup.base_url + "/" });
  await waitFor("browser navigation", async () => await evaluate("location.origin") === startup.base_url);

  const note = (await invoke("notes.create", {
    space_id: space.id, title: "Latency fixture", markdown: "one two three\n",
  })).data?.note;
  if (!note?.id) throw new Error("Fixture note creation failed");
  const notesActivation = await invoke("plugins.enable", {
    plugin_id: "calternal-notes-insights", space_id: space.id,
  });
  if (!notesActivation.data) throw new Error("Fixture plugin enable failed: " + notesActivation.error?.code);
  let operation = "notes-insights.summarize";
  let input = { id: note.id };
  let expected = 'Row([Metric("Words", "3"), Metric("Characters", "14"), Metric("Lines", "1")])';
  const warm = () => waitFor("enrolled worker warming", async () => {
    const { data } = await invoke(operation, input);
    return data?.view?.source === expected;
  });
  await warm();

  const settings = process.env.CALTERNAL_PERF_SETTINGS_FIXTURE;
  let invalidation;
  if (settings) {
    const source = resolve(settings);
    const review = cli(["plugin", "review", source]);
    cli(["plugin", "install", source, "--appdata", join(root, "appdata"),
      "--approve", review.checksum, "--space", space.id, "--account", space.owner]);
    const activation = await invoke("plugins.enable", { plugin_id: "composer-settings", space_id: space.id });
    if (!activation.data) throw new Error("Composer fixture enable failed: " + activation.error?.code);
    const address = { space_id: space.id, plugin_id: "composer-settings", key: "label" };
    invalidation = await measureSettingsFeed(startup.base_url, token, address, invoke);
    operation = "composer-settings.label";
    input = { space_id: space.id };
    expected = 'Badge("Timing label 34")';
    await warm();
  }
  const measured = await evaluate(`(${measure.toString()})(...${JSON.stringify([token, operation, input, expected])})`);
  const sorted = [...measured.samples].sort((a, b) => a - b);
  if (sorted.length !== 100 || sorted.some(value => !Number.isFinite(value) || value < 0)) {
    throw new Error("Expected 100 valid browser samples");
  }
  const p95 = sorted[94];
  const passed = settings ? undefined : p95 <= limit.budget;
  const orderedInvalidation = invalidation?.toSorted((a, b) => a - b);
  const report = {
    metric: settings ? "plugin.warm_composer_settings" : metric,
    operation, browser: measured.browser, n: sorted.length,
    p50: sorted[49], p95, budget: settings ? undefined : limit.budget, unit: limit.unit, passed,
    invalidation: orderedInvalidation && { n: orderedInvalidation.length,
      p50: orderedInvalidation[14], p95: orderedInvalidation[28] },
  };
  writeFileSync(reportPath, JSON.stringify({ ...report, samples: measured.samples, invalidationSamples: invalidation }, null, 2) + "\n");
  console.log(JSON.stringify(report));
  if (passed === false) process.exitCode = 1;
} catch (error) {
  for (const [name, log] of logs) {
    const safe = log.split("\n").filter(line => !/token|code=|bearer/i.test(line));
    console.error(name, safe.slice(-12).join("\n"));
  }
  throw error;
} finally {
  socket?.close();
  for (const child of children.reverse()) {
    if (child.exitCode !== null || child.signalCode !== null || !child.pid) continue;
    const stopped = once(child, "exit");
    child.kill("SIGTERM");
    await Promise.race([stopped, sleep(1500, undefined, { ref: false })]);
    if (child.exitCode === null && child.signalCode === null) {
      child.kill("SIGKILL");
      await stopped;
    }
  }
  const containers = spawnSync(podman, [
    "ps", "-aq", "--filter", "label=org.calternal.sandbox-root=" + join(machine, "sandboxes"),
  ], { encoding: "utf8", timeout: 8000 });
  const ids = (containers.stdout || "").trim().split(/\s+/).filter(Boolean);
  const removed = ids.length
    ? spawnSync(podman, ["rm", "-f", "-t", "0", "--ignore", ...ids], { timeout: 8000 })
    : { status: 0 };
  if (containers.status !== 0 || removed.status !== 0) {
    console.error("Fixture container cleanup failed for " + machine);
    process.exitCode = 2;
  }
  rmSync(root, { recursive: true, force: true });
}
