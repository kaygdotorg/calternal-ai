import { openRealServerPage, signUpRealServerOwner, startRealServer } from "./lib.mjs";
// A plugin job's durable wait against a real server of this tree (docs/plugin-api.md, "Durable job waits"): the test-only `jobs-demo` plugin parks a job on a question and on an approval; the Inbox lists each as a row of its own (no thread), pressing it opens the prompt in a sheet, answering it
// resumes the job (it ends `done`) and the row leaves. Needs a server built with the `test-plugins` feature (the default debug build) and `node` on the PATH for the direct worker. Env: CAL_BIN, CAL_DIST, CAL_PLUGINS, CAL_TEST_PLUGINS (tests/plugins), CAL_JOB_PORT.
import { resolve } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_JOB_PORT ?? 3415);
let bad = 0, server, url;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const op = (p, name, input) => p.evaluate(async ([name, input]) => { const r = await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: name, input }) }); return r.json(); }, [name, input]);
try {
  server = await startRealServer(PORT, { args: ["--test-plugin-root", process.env.CAL_TEST_PLUGINS ?? resolve("../../tests/plugins")], env: { CALTERNAL_WORKER_TEST_BACKEND: "direct" } }); url = server.url;
  const b = await chromium.launch();
  const p = await openRealServerPage(b);
  await signUpRealServerOwner(p, server, "Prompt Owner");
  const space = (await op(p, "spaces.list", {})).data.spaces.find((s) => s.personal).id;
  const en = await op(p, "plugins.enable", { space_id: space, plugin_id: "jobs-demo" });
  console.log("   plugins.enable:", JSON.stringify(en).slice(0, 160));
  const deadline = Date.now() + 3600_000;
  // The first start spawns the plugin worker: on a busy machine it can answer "extension timed out"; a retry starts a fresh one.
  const start = async (wait) => { let r; for (let i = 0; i < 4; i++) { r = await op(p, "jobs-demo.start", { space_id: space, steps: 1, delay_ms: 0, every: 1, wait, deadline_ms: deadline }); if (r.ok) break; await p.waitForTimeout(2000); } return r; };
  const q = await start("question");
  const a = await start("approval");
  console.log("   jobs:", JSON.stringify(q).slice(0, 140), JSON.stringify(a).slice(0, 140));
  let wire = []; for (let i = 0; i < 60; i++) { wire = (await op(p, "requests.list", {})).data?.requests ?? []; if (wire.filter((r) => r.job_id).length >= 2) break; await p.waitForTimeout(500); }
  ok(wire.filter((r) => r.job_id && !r.thread_id).length === 2, "the server lists both waits as requests with a job and no thread", JSON.stringify(wire.map((r) => [r.detail?.kind, !!r.job_id, r.thread_id])));
  p.on("pageerror", (e) => console.log("   pageerror:", String(e).slice(0, 300))); p.on("console", (m) => { console.log("   console." + m.type() + ":", m.text().slice(0, 300)); });
  await p.goto(`${url}/threads?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2500);
  const rows = p.locator("[role=tabpanel] ul[aria-label='Waiting for you'] li[data-job]");
  for (let i = 0; i < 24 && (await rows.count()) < 2; i++) await p.waitForTimeout(500);
  
  ok((await rows.count()) === 2, "the Inbox shows both as rows of their own", `${await rows.count()} | ${(await p.locator("aside.side").innerText().catch(() => "")).replace(/\s+/g, " ").slice(0, 200)}`);
  const question = rows.filter({ hasText: /Release selection|Which release/ }).first();
  await question.locator("button, a").first().click();
  const sheet = p.getByRole("dialog", { name: /from a plugin/ });
  await sheet.waitFor({ timeout: 5000 }).catch(() => {});
  ok((await sheet.count()) === 1 && /Which release/.test(await sheet.innerText()), "pressing the question opens it in a sheet with the plugin's form", (await sheet.innerText().catch(() => "")).replace(/\s+/g, " ").slice(0, 120));
  await sheet.getByRole("textbox").first().fill("v1.2");
  await sheet.getByRole("button", { name: /Send/ }).first().click();
  await p.waitForTimeout(1500);
  ok((await p.getByRole("dialog", { name: /from a plugin/ }).count()) === 0 && (await rows.count()) === 1, "answering it closes the sheet and the row leaves");
  const jobs = await op(p, "jobs.get", { job_id: q.data?.job_id });
  let state = jobs.data?.job?.state; for (let i = 0; i < 20 && state !== "done"; i++) { await p.waitForTimeout(500); state = (await op(p, "jobs.get", { job_id: q.data?.job_id })).data?.job?.state; }
  ok(state === "done", "the job that waited ends done once it is answered", String(state));
  await rows.first().locator("button, a").first().click();
  await p.getByRole("dialog", { name: /from a plugin/ }).getByRole("button", { name: /Allow once|Deny/ }).first().waitFor({ timeout: 5000 }).catch(() => {});
  ok((await p.getByRole("dialog", { name: /from a plugin/ }).getByText(/Drawn by Calternal/).count()) === 1, "an approval is drawn by the shell, with Allow and Deny");
  await p.getByRole("dialog", { name: /from a plugin/ }).getByRole("button", { name: "Deny" }).click(); await p.waitForTimeout(1500);
  ok((await rows.count()) === 0, "denying it takes the last row away");
  await b.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
finally { await server?.stop(); }
process.exit(bad ? 1 : 0);
