// axe-core over the main screens in a real browser (contrast included), light and dark, desktop and phone. Exits 1 on any violation.
import AxeBuilder from "@axe-core/playwright";
import { launch, open, serve } from "./lib.mjs";

const screens = [
  ["threads running", "/threads/t-run?panel=changes"],
  ["threads question", "/threads/t-wait?panel=run"],
  ["threads secret", "/threads/t-secret?panel=files"],
  ["threads long", "/threads/t-long?panel=run"],
  ["notes", "/notes/n-1?panel=inspector"],
  ["settings appearance", "/settings/appearance"],
  ["settings agents", "/settings/agents"],
  ["settings keyboard", "/settings/keyboard"],
  ["settings machines", "/settings/machines"],
  ["settings plugins", "/settings/plugins"],
  ["settings account", "/settings/account"],
  ["settings members", "/settings/members"],
  ["settings invites", "/settings/invites"],
  ["settings secrets", "/settings/secrets"],
  ["settings space", "/settings/instructions"],
  ["settings space plugins", "/settings/space-plugins"],
  ["settings groups", "/settings/plans"],
  ["settings pool", "/settings/pool"],
  ["settings storage", "/settings/storage"],
  ["appearance sheet", "/settings/appearance?sheet=1"],
  ["first run", "/threads/t-idle?firstrun=1"],
  ["notes empty", "/notes"],
  ["threads list", "/threads"],
  ["not found", "/nothing/here"],
];
const stop = await serve();
const browser = await launch();
let bad = 0;
try {
  // `node e2e/a11y.mjs [device [scheme]]` checks one part (the gate runs the four parts at once); no arguments checks all four.
const [onlyDevice, onlyScheme] = process.argv.slice(2);
for (const device of ["desktop", "phone"]) for (const scheme of ["light", "dark"]) for (const [name, path] of screens) {
    if ((onlyDevice && device !== onlyDevice) || (onlyScheme && scheme !== onlyScheme)) continue;
    const { page, ctx } = await open(browser, device, scheme, path);
    const r = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "best-practice"]).analyze();
    const v = r.violations.map((x) => `${x.id} (${x.nodes.length}): ${x.nodes[0].target.join(" ")} :: ${x.nodes[0].failureSummary?.split("\n")[1] ?? ""}`);
    console.log(`${v.length ? "FAIL" : "ok  "} ${device} ${scheme} ${name}${v.length ? "\n   " + v.join("\n   ") : ""}`);
    bad += v.length;
    await ctx.close();
  }
} finally {
  await browser.close();
  stop();
}
process.exit(bad ? 1 : 0);
