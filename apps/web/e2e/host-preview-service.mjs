// Test-only control for the disposable unit created by smoke-dev-chat-preview.sh.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { setTimeout } from "node:timers/promises";

export const previewUnit = process.env.HOST_PREVIEW_UNIT;
if (previewUnit) assert.match(previewUnit, /^calternal-preview-smoke-[a-zA-Z0-9]+\.service$/);

export function service(action) {
  execFileSync("systemctl", ["--user", action, previewUnit], { stdio: "inherit" });
}

export function startPreview(directory, origin) {
  const release = process.env.HOST_PREVIEW_RELEASE;
  const scratch = path.dirname(directory);
  execFileSync("systemd-run", [
    "--user", "--quiet", `--unit=${previewUnit}`,
    "--property=After=network.target",
    `--property=WorkingDirectory=${scratch}`,
    "--setenv=PATH=/nix/var/nix/profiles/default/bin:/usr/local/bin:/usr/bin:/bin",
    `--setenv=CALTERNAL_WORKER_SDK=${release}/sdk`,
    `--property=EnvironmentFile=${scratch}/runtime.env`,
    "--property=UnsetEnvironment=CALTERNAL_REAL_RUN_IMAGE CALTERNAL_RUN_SIGN_IN_DIR",
    "--property=Restart=on-failure", "--property=RestartSec=3", "--property=UMask=0077",
    process.env.HOST_BIN, "preview-serve", directory, `${release}/web`,
    `${release}/calternal-chat`, `${release}/calternal-chat-claude-code`, new URL(origin).port,
  ], { stdio: "inherit" });
}

export async function previewReady(origin) {
  for (let attempt = 0; attempt < 150; attempt++) {
    try {
      if ((await fetch(`${origin}/v1/auth/status`, { signal: AbortSignal.timeout(500) })).ok) return;
    } catch {
      // The user service opens its listener after initializing storage and Podman.
    }
    await setTimeout(100);
  }
  throw new Error("preview user service did not listen");
}

export async function enterSetupCode(page, directory) {
  // This code belongs to this suite's throwaway account. Passing it through
  // evaluate keeps the value out of Playwright's action log.
  const code = await readFile(path.join(directory, "first-owner-code"), "utf8");
  await page.locator('input[name="token"]').evaluate((input, value) => {
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }, code);
}
