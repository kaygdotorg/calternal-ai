// Runs one e2e script against the built app served on a free port (not the fixed 4174, which another lane's preview may hold): `node e2e/with-preview.mjs e2e/<script>.mjs [args]`. The script gets CAL_URL.
import { spawn } from "node:child_process";
import { createServer } from "node:net";
const port = await new Promise((res) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); });
const url = `http://127.0.0.1:${port}`;
const srv = spawn("npx", ["vite", "preview", "--port", String(port), "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: true });
const stop = () => { try { process.kill(-srv.pid); } catch { /* gone */ } };
process.on("exit", stop);
for (let i = 0; i < 120; i++) { try { if ((await fetch(url)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const p = spawn("node", process.argv.slice(2), { stdio: "inherit", env: { ...process.env, CAL_URL: url } });
p.on("close", (c) => { stop(); process.exit(c ?? 1); });
