// Through the preview host, up to the ceremony and no further (the setup token belongs to the owner): the page shows setup, the API answers behind the proxy, headers and cookies are as expected.
import { chromium } from "playwright";
const BASE = process.env.CAL_URL ?? "https://preview.calternal.cloud";
let failed = 0;
const check = (ok, what, extra = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${what} ${extra}`); if (!ok) failed++; };
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 1200, height: 800 } });
const page = await ctx.newPage();
const errs = [];
page.on("console", (m) => m.type() === "error" && errs.push(m.text().slice(0, 160)));
await page.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
const st = await page.request.get(`${BASE}/api/v1/auth/status`);
const body = await st.json();
console.log("status", JSON.stringify(body), "cache-control:", st.headers()["cache-control"], "csp:", st.headers()["content-security-policy"] ?? "none");
const resp = await page.goto(`${BASE}/ai/?server=1`);
console.log("page csp:", resp.headers()["content-security-policy"] ?? "none");
await page.waitForSelector("html[data-ready]");
check(await page.getByRole("heading", { name: body.setup ? "Set up Calternal" : "Sign in" }).isVisible(), `shows ${body.setup ? "first-owner setup" : "sign in"} behind the proxy`);
check(!(await page.getByRole("button", { name: /Create passkey|Sign in with a passkey/ }).isEnabled().catch(() => true)) || !body.setup, "setup button disabled until code and name are given");
// login/start does not consume anything: options must name the preview RP.
const ls = await page.request.post(`${BASE}/api/v1/auth/login/start`, { data: {}, headers: { origin: BASE } });
const lj = await ls.json().catch(() => ({}));
console.log("login/start", ls.status(), "rpId:", lj.options?.publicKey?.rpId, "set-cookie:", ls.headers()["set-cookie"] ?? "none");
check(ls.status() === 200 && lj.options?.publicKey?.rpId === "preview.calternal.cloud", "login/start answers with the preview RP ID");
console.log("cookies after load:", JSON.stringify((await ctx.cookies()).map((c) => c.name)));
console.log("console errors:", JSON.stringify(errs));
await b.close();
process.exit(failed ? 1 : 0);
