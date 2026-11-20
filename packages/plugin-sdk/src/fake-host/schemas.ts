/** Contract schema validation for the fake host, compiled once per process. */
import { readFileSync } from "node:fs";
import Ajv2020, { type ValidateFunction } from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import { HostRefusal } from "./refusal.js";

const contract = new URL("../../generated/contract/", import.meta.url);
const catalogueId = "https://calternal.cloud/contract/host-catalogue-v2.json";

function load(name: string): Record<string, unknown> {
  const document = JSON.parse(readFileSync(new URL(name, contract), "utf8")) as Record<string, unknown>;
  // Draft 7 and 2020-12 documents share one registry; the keywords they use agree.
  delete document.$schema;
  return document;
}

let shared: ContractSchemas | undefined;

/**
 * Validators for catalogue definitions and author-declared schemas. Author
 * schemas resolve `host-catalogue-v2.json#/$defs/...` against the pinned
 * catalogue. Compiled validators are cached by key.
 */
export class ContractSchemas {
  private ajv = new Ajv2020({ strict: false, allErrors: true });
  private cache = new Map<string, ValidateFunction>();
  /** The parsed pinned catalogue. */
  readonly catalogue = load("host-catalogue-v2.json");

  /** One shared instance; compiling the catalogue costs about a second. */
  static get(): ContractSchemas {
    return shared ??= new ContractSchemas();
  }

  private constructor() {
    addFormats(this.ajv);
    for (const bits of [8, 32]) {
      this.ajv.addFormat(`uint${bits}`, { type: "number", validate: (value: number) => Number.isInteger(value) && value >= 0 && value < 2 ** bits });
    }
    this.ajv.addSchema(this.catalogue);
    this.ajv.addSchema(load("description-v2.schema.json"));
  }

  /** Validate against one catalogue pointer such as `#/$defs/ItemCreate`; throws `400 invalid_shape`. */
  check(pointer: string, value: unknown, what: string): void {
    const key = catalogueId + pointer;
    let validate = this.cache.get(key);
    if (!validate) {
      validate = this.ajv.getSchema(key);
      if (!validate) throw new Error(`The pinned catalogue has no ${pointer}.`);
      this.cache.set(key, validate);
    }
    this.run(validate, value, what);
  }

  /** Validate against an author-declared schema cached under `key`; throws `400 invalid_shape`. */
  checkDeclared(key: string, schema: unknown, value: unknown, what: string): void {
    let validate = this.cache.get(key);
    if (!validate) {
      const document = typeof schema === "boolean" ? schema
        : { ...(schema as object), $id: `https://calternal.cloud/contract/declared-${key.replace(/[^a-z0-9.-]/gi, "_")}.json` };
      validate = this.ajv.compile(document);
      this.cache.set(key, validate);
    }
    this.run(validate, value, what);
  }

  private run(validate: ValidateFunction, value: unknown, what: string): void {
    if (validate(value)) return;
    throw new HostRefusal(400, "invalid_shape", `${what} does not match its schema: ${this.ajv.errorsText(validate.errors, { dataVar: "value" })}.`);
  }
}
