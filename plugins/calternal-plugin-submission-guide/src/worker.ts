/** Provide the gallery submission checklist as read-only Markdown. */
import { serveWorker } from "@calternal/plugin-sdk";
import manifest from "../manifest.json" with { type: "json" };
import markdown from "../README.md";

serveWorker(manifest, {
  "calternal-plugin-submission-guide.explain": async () => ({ markdown }),
}, process.stdin, line =>
  new Promise<void>((resolve, reject) =>
    process.stdout.write(line, error => error ? reject(error) : resolve())),
).catch(() => {
  process.stderr.write("Submission guide worker protocol error.\n");
  process.exitCode = 1;
  process.stdin.destroy();
});
