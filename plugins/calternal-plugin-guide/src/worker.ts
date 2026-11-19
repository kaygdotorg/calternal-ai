/** Serve the local guide and publish its documents through the current space broker. */
import { serveWorker, validateManifestV1, type WorkerHandlerV2 } from "@calternal/plugin-sdk";
import reference from "../reference.md";
import walkthrough from "../walkthrough.md";
import skill from "../skills/plugin-authoring/SKILL.md";
import manifest from "../manifest.json" with { type: "json" };
import starter from "../../../packages/plugin-sdk/templates/manifest.json" with { type: "json" };

const starterJson = JSON.stringify(starter);

const waitHeading = "### Wait for a signal";
const jobsHeading = "## Durable jobs and progress";
const sectionFrom = (heading: string) => reference.slice(reference.indexOf(heading));
const content = new Map([
  ["reference", reference], ["walkthrough", walkthrough], ["skill", skill],
  ["jobs", sectionFrom(jobsHeading)], ["waits", sectionFrom(waitHeading)],
]);

function field(input: unknown, key: string): unknown {
  return input && typeof input === "object" && Object.hasOwn(input, key)
    ? (input as Record<string, unknown>)[key] : undefined;
}

const handlers = {
  "calternal-plugin-guide.explain": async input => {
    const topic = field(input, "topic");
    const markdown = typeof topic === "string" ? content.get(topic) : undefined;
    if (markdown === undefined) throw new Error("unknown topic");
    return { markdown };
  },
  "calternal-plugin-guide.scaffold": async input => {
    const name = field(input, "name");
    if (typeof name !== "string" || !/^[a-z][a-z0-9-]{0,63}$/.test(name)) throw new Error("invalid name");
    const manifest = validateManifestV1(JSON.parse(starterJson.replaceAll("__PLUGIN__", name)));
    return { manifest, command: `calternal plugin new ${name}` };
  },
  "calternal-plugin-guide.check": async input => {
    try {
      const checked = validateManifestV1(field(input, "manifest"));
      return { valid: true, host_calls: checked.host_calls, operations: checked.operations.map(op => ({
        id: op.id, risk: op.risk, capability: op.required_capability,
      })) };
    } catch {
      return { valid: false, message: "Check the manifest version, identifiers, permissions and risk declarations. Run calternal plugin check for host schema validation." };
    }
  },
  "calternal-plugin-guide.publish": async (_, context) => ({
    item: await context.call("write_item", {
      kind: "plugin-guides",
      metadata: { title: "Calternal Plugin Guide" },
      text: `${reference.trimEnd()}\n\n${walkthrough.trimEnd()}\n`,
    }),
  }),
} satisfies Record<string, WorkerHandlerV2>;

serveWorker(manifest, handlers, process.stdin, line =>
  new Promise<void>((resolve, reject) =>
    process.stdout.write(line, error => error ? reject(error) : resolve())),
).catch(() => {
  process.stderr.write("Calternal Plugin Guide worker protocol failure\n");
  process.exitCode = 1;
  process.stdin.destroy();
});
