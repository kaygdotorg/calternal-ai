// Shared by the suites: the sealed fixtures on disk and an ajv that validates them against the two sealed schemas.
import Ajv2020 from "ajv/dist/2020";
import addFormats from "ajv-formats";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

export const contractDir = join(__dirname, "../../../contract");
export const fixturesDir = join(contractDir, "fixtures/description-v2");
const readJson = (p: string) => JSON.parse(readFileSync(p, "utf8"));

/** One fixture file: its name, parsed JSON, and which schema entry point it belongs to (the verifier's rule: event-*, reply-*, else the app or view description). */
export interface Fixture { name: string; path: string; doc: unknown; kind: "event" | "reply" | "description"; invalid: boolean; dir: string }

export function loadFixtures(): Fixture[] {
  const out: Fixture[] = [];
  const add = (dir: string, name: string) => {
    const path = join(dir, name); if (!name.endsWith(".json") || name === "expected-errors.json") return; // (expected-errors.json lists what the verifier must say about the invalid ones: not a fixture)
    // A fixture that is not even JSON (the verifier's strict loader refuses NaN and duplicate keys) can only be an invalid one: it stays in the list as a document no schema accepts.
    let doc: unknown; try { doc = readJson(path); } catch { doc = Symbol.for("not-json"); }
    out.push({ name, path, dir, doc, invalid: name.startsWith("invalid-"), kind: /^(invalid-)?event-/.test(name) ? "event" : /^(invalid-)?reply-/.test(name) ? "reply" : "description" });
  };
  for (const n of readdirSync(fixturesDir)) add(fixturesDir, n);
  for (const d of readdirSync(join(fixturesDir, "events"), { withFileTypes: true })) if (d.isDirectory()) for (const n of readdirSync(join(fixturesDir, "events", d.name))) add(join(fixturesDir, "events", d.name), n);
  return out;
}

/** An ajv holding both schemas; `validators` are the entry points. */
export function makeValidators() {
  const ajv = new Ajv2020({ strict: false, allErrors: false });
  addFormats(ajv);
  const description = readJson(join(contractDir, "description-v2.schema.json")), catalogue = readJson(join(contractDir, "host-catalogue-v2.json"));
  ajv.addSchema(catalogue, "host-catalogue-v2.json"); // the relative citations resolve against this name too
  ajv.addSchema(description);
  const id = description.$id as string;
  return {
    description: ajv.getSchema(id)!,
    event: ajv.getSchema(`${id}#/$defs/Event`)!,
    reply: ajv.getSchema(`${id}#/$defs/WindowReply`)!,
  };
}
