/** Negative tests prove the production gates reject undocumented items and broken links. */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, unlinkSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Run a gate with a finite timeout and require an actual documentation diagnostic. */
function rejected(command, args, diagnostic) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8", timeout: 60000 });
  assert.ifError(result.error);
  assert.notEqual(result.status, 0, "the documentation gate unexpectedly accepted the probe");
  assert.match(result.stdout + result.stderr, diagnostic);
}

for (const [name, source] of [
  ["undocumented TypeScript export", "export const documentationGateProbe = 1;\n"],
  ["undocumented TypeScript member", "/** Negative test container. */\nexport interface DocumentationGateProbe { missing: string; }\n"],
]) {
  test(name, () => {
    const probe = path.join(root, "packages/plugin-sdk/src/__documentation_gate_probe.ts");
    // Exclusive creation prevents overwriting an existing file in the shared checkout.
    writeFileSync(probe, source, { flag: "wx" });
    try {
      rejected("npm", ["run", "check", "--prefix", "scripts/docs-tools"], /(?:documentationGateProbe|DocumentationGateProbe\.missing).*does not have any documentation/);
    } finally {
      unlinkSync(probe);
    }
  });
}

for (const [name, source, cargo, diagnostic] of [
  ["undocumented Rust public item", "\npub const DOCUMENTATION_GATE_PROBE: u8 = 0;\n", "check", /missing documentation for a constant/],
  ["broken Rust reference link", "\n/// See [`DocumentationGateMissingTarget`].\npub const DOCUMENTATION_GATE_PROBE: u8 = 0;\n", "doc", /unresolved link to `DocumentationGateMissingTarget`/],
]) {
  test(name, () => {
    const filename = path.join(root, "crates/core/src/lib.rs");
    const original = readFileSync(filename, "utf8");
    writeFileSync(filename, original + source);
    try {
      const args = ["cargo", cargo, "--locked", "-j4", "-p", "calternal-core"];
      if (cargo === "doc") args.push("--no-deps", "--target-dir", "reference-docs/rust");
      rejected("./scripts/with-toolchain.sh", args, diagnostic);
    } finally {
      writeFileSync(filename, original);
    }
  });
}
