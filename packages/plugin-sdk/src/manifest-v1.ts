/** Data-only manifest v1 validation; requests never issue grants.
 * @module
 */
import type { ManifestV1 } from "@calternal/contracts";

const id = /^[a-z0-9][a-z0-9._-]{0,79}$/;
const capabilities = new Set(["space.read", "content.write", "plugins.use", "plugins.manage", "members.invite", "members.manage", "managers.manage", "space.settings", "space.transfer", "space.delete", "spaces.list", "spaces.create"]);
/** Closed broker method vocabulary; declaration never grants permission. */
export type HostCallKind = "read_item" | "write_item" | "list_collection" | "emit_event" | "start_job" | "request_secret_handle";
const hostCalls = new Set(["read_item", "write_item", "list_collection", "emit_event", "start_job", "request_secret_handle"]);
function fail(): never { throw new Error("invalid manifest v1"); }
function record(v: unknown, keys: string[]): Record<string, unknown> {
  if (!v || typeof v !== "object" || Array.isArray(v)) return fail();
  const r = v as Record<string, unknown>;
  if (Object.keys(r).length !== keys.length || keys.some(k => !Object.hasOwn(r, k))) return fail();
  return r;
}
function identifier(v: unknown): string { if (typeof v !== "string" || !id.test(v)) return fail(); return v; }
function text(v: unknown, max = 256): string { if (typeof v !== "string" || !v.trim() || new TextEncoder().encode(v).length > max) return fail(); return v; }
function schema(v: unknown): void { if (typeof v !== "boolean" && (!v || typeof v !== "object" || Array.isArray(v))) fail(); }
/** Validate the strict data-only menu, references, bounds, defaults and retry consistency.
 * Schemas remain data and must be compiled by the trusted execution adapter before activation.
 * Cost: O(menu bytes + declaration count), bounded sets and serialization, no I/O or execution.
 * @returns The validated menu without cloning or executing plugin code.
 */
export function validateManifestV1(value: unknown): ManifestV1 {
  if (new TextEncoder().encode(JSON.stringify(value)).length > 256 * 1024) fail();
  const m = record(value, ["id", "name", "icon", "version", "api_version", "kinds", "operations", "collections", "views", "attention", "settings", "schedules", "webhooks", "secrets", "hooks", "themes", "host_calls"]);
  identifier(m.id); identifier(m.icon); text(m.name); text(m.version, 80);
  if (m.api_version !== 1) fail();
  function rows(key: string, keys: string[], visit: (r: Record<string, unknown>) => void): void {
    const list = m[key]; if (!Array.isArray(list) || list.length > 128) return fail();
    const seen = new Set<string>();
    for (const v of list) { const r = record(v, keys); const key = identifier(r.id); if (seen.has(key)) fail(); seen.add(key); visit(r); }
  }
  const operations = new Set<string>();
  rows("kinds", ["id", "schema", "storage", "claimed_text"], r => {
    schema(r.schema); if (typeof r.claimed_text !== "boolean") fail();
    if (!r.storage || typeof r.storage !== "object" || Array.isArray(r.storage)) fail();
    const placement = r.storage as Record<string, unknown>;
    if (Object.hasOwn(placement, "folder")) {
      record(placement, ["folder"]); const folder = text(placement.folder, 255);
      if (folder !== folder.trim() || folder.startsWith(".") || /[/\\:\p{Cc}]/u.test(folder) || folder.normalize("NFC") !== folder || folder.toLowerCase() === "trash") fail();
    } else { record(placement, ["hidden"]); if (placement.hidden !== true) fail(); }
  });
  rows("operations", ["id", "input_schema", "output_schema", "required_capability", "mutates", "idempotency"], r => {
    operations.add(identifier(r.id)); schema(r.input_schema); schema(r.output_schema);
    if (!capabilities.has(String(r.required_capability)) || typeof r.mutates !== "boolean" || !["read_only", "request_id", "never"].includes(String(r.idempotency)) || r.mutates === (r.idempotency === "read_only") || (r.mutates && r.required_capability === "space.read")) fail();
  });
  const op = (r: Record<string, unknown>): void => { if (!operations.has(String(r.operation))) fail(); };
  rows("collections", ["id", "kind", "name"], r => { identifier(r.kind); text(r.name); });
  rows("views", ["id", "region", "component"], r => { identifier(r.component); if (!["sidebar", "main", "right_card"].includes(String(r.region))) fail(); });
  rows("attention", ["id", "operation"], op);
  rows("settings", ["id", "value_type", "default"], r => { if (!["string", "boolean", "number"].includes(String(r.value_type)) || typeof r.default !== r.value_type || (typeof r.default === "number" && !Number.isFinite(r.default))) fail(); });
  rows("schedules", ["id", "operation", "expression"], r => { op(r); text(r.expression); });
  rows("webhooks", ["id", "operation"], op);
  rows("hooks", ["id", "mode", "patterns", "operation"], r => { op(r); if (!["watch", "block"].includes(String(r.mode)) || !Array.isArray(r.patterns) || !r.patterns.length || r.patterns.length > 128) fail(); for (const p of r.patterns as unknown[]) identifier(typeof p === "string" && p.endsWith(".*") ? p.slice(0, -2) : p); });
  rows("themes", ["id", "stylesheet"], r => { const p = text(r.stylesheet, 4096); if (!p.endsWith(".css") || /[:\\]/.test(p) || p.split("/").some(x => !x || x === "." || x === "..")) fail(); });
  for (const key of ["secrets", "host_calls"]) { const list = m[key]; if (!Array.isArray(list) || list.length > 128 || new Set(list).size !== list.length) fail(); for (const v of list as unknown[]) { identifier(v); if (key === "host_calls" && !hostCalls.has(String(v))) fail(); } }
  return value as ManifestV1;
}
