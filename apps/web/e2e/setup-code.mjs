// The first screen of a fresh installation: no owner yet (`auth/status` says `setup: true`) and a well-formed code the server does not know. The server answers that with `invite_invalid`
// (the invitation code), and the first owner must still see the first owner's message, naming the command that makes a code, never the invitation words and never a file. The server is mocked
// at the HTTP edge (only status, describe and the refusal), so this runs on the built app alone. Usage: node e2e/with-preview.mjs e2e/setup-code.mjs
import { BASE, launch, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const json = (route, body, status = 200) => route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
try {
  for (const device of ["desktop", "phone"]) {
    const c2 = await browser.newContext({ viewport: device === "phone" ? { width: 390, height: 844 } : { width: 1200, height: 800 } });
    const p = await c2.newPage();
    await p.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    await p.route("**/api/v1/auth/status", (r) => json(r, { setup: true, signed_in: false }));
    await p.route("**/api/v1/describe", (r) => json(r, { protocol_version: 1, operations: [], kinds: [] }));
    await p.route("**/api/v1/auth/setup/start", (r) => json(r, { error: { code: "invite_invalid", message: "unknown code" } }, 403));
    await p.goto(BASE + "/?server=1");
    await p.waitForSelector("html[data-ready]");
    ok(await p.getByRole("heading", { name: "Set up Calternal" }).isVisible(), `${device}: setup is pending, so the first-owner screen shows`);
    const inputs = p.locator("form input");
    await inputs.nth(0).fill("ABCD-EFGH-JKLM"); if ((await inputs.count()) > 1) await inputs.nth(1).fill("Kay");
    await p.getByRole("button", { name: /Create passkey|Set up/ }).click();
    await p.waitForTimeout(600);
    const text = await p.locator("form").innerText();
    ok(/podman exec calternal-ai \/usr\/local\/bin\/calternal setup-code --appdata \/data\/appdata/.test(text), `${device}: an unknown well-formed code shows the setup-code command`, text.slice(-160).replace(/\n/g, " "));
    ok(/24 hours/.test(text) && !/invitation|invited|setup\.token/i.test(text), `${device}: it says the code lasts 24 hours, and never the invitation words or the file`);
    await c2.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
