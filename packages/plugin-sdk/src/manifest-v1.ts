/** Data-only manifest v1 validation; requests never issue grants.
 * @module
 */
import type { HostCallKind, ManifestV1 } from "@calternal/contracts";
import { SPACE_INSTRUCTIONS_PATH } from "./space-paths.js";
const reservedInstructionParts=SPACE_INSTRUCTIONS_PATH.toLowerCase().split("/");

const id = /^[a-z0-9][a-z0-9._-]{0,79}$/;
const capabilities = new Set(["space.read", "content.write", "plugins.use", "plugins.manage", "members.invite", "members.manage", "managers.manage", "space.settings", "space.transfer", "space.delete", "spaces.list", "spaces.create"]);
/** Closed broker method vocabulary; declaration never grants permission. */
export type { HostCallKind } from "@calternal/contracts";
const hostCalls = new Set<HostCallKind>(["read_item", "list_items_v1", "read_setting_v1", "content_prepare_v1", "write_item", "list_collection", "emit_event", "start_job", "checkpoint_job", "request_secret_handle"]);

/** Check a manifest or frame's method against the same closed vocabulary. No I/O. */
export function isHostCallKind(value: unknown): value is HostCallKind {
  return typeof value === "string" && hostCalls.has(value as HostCallKind);
}
function fail(): never { throw new Error("invalid manifest v1"); }
function record(v: unknown, keys: string[], optional: string[] = []): Record<string, unknown> {
  if (!v || typeof v !== "object" || Array.isArray(v)) return fail();
  const r = v as Record<string, unknown>;
  if (keys.some(k => !Object.hasOwn(r, k)) || Object.keys(r).some(k => !keys.includes(k) && !optional.includes(k))) return fail();
  return r;
}
/** An operation target names a kind and a required string property of the input schema. */
function target(v: unknown, input: unknown): void {
  if (v === undefined || v === null) return;
  const t = record(v, ["kind", "field"]); identifier(t.kind);
  const field = t.field;
  const s = input as { type?: unknown; properties?: Record<string, { type?: unknown }>; required?: unknown };
  if (typeof field !== "string" || !/^[a-z0-9_]{1,64}$/.test(field) || field === "space_id" || !s || typeof s !== "object" || s.type !== "object" || s.properties?.[field]?.type !== "string" || !Array.isArray(s.required) || !s.required.includes(field)) fail();
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
  const m = record(value, ["id", "name", "icon", "version", "api_version", "operations", "collections", "views", "attention", "settings", "schedules", "webhooks", "secrets", "hooks", "themes", "host_calls"], ["provider", "slots", "owned_slots", "palette", "kinds", "network"]);
  if (m.provider !== undefined) {
    const p = record(m.provider, ["protocol_version", "executable", "default_model", "models", "sign_in"]);
    if (p.sign_in !== undefined && typeof p.sign_in !== "boolean") throw new Error("invalid provider sign_in");
    if (p.protocol_version !== 1 || typeof p.executable !== "string" || !/^[a-z0-9][a-z0-9_-]{0,79}$/.test(p.executable)) fail();
    const modelId = /^[a-zA-Z0-9][a-zA-Z0-9._:@/-]{0,127}$/;
    const ids = new Set<string>();
    if (!Array.isArray(p.models) || p.models.length > 128) fail();
    for (const value of p.models) {
      const model = record(value, ["id", "name", "reasoning_levels"]);
      const name = text(model.name, 128);
      const id = text(model.id, 128);
      const levels = model.reasoning_levels;
      if (!modelId.test(id) || ids.has(id) || /\p{Cc}/u.test(name) ||
          !Array.isArray(levels) || levels.length > 16 || new Set(levels).size !== levels.length ||
          levels.some(level => typeof level !== "string" || level.length > 32 || !modelId.test(level))) fail();
      ids.add(id);
    }
    if (!ids.has(text(p.default_model, 128))) fail();
  }
  identifier(m.id); identifier(m.icon); text(m.name); text(m.version, 80);
  if (m.api_version !== 1) fail();
  function rows(key: string, keys: string[], visit: (r: Record<string, unknown>) => void, optional: string[] = []): void {
    const list = key === "kinds" && !Object.hasOwn(m, key) ? [] : m[key]; if (!Array.isArray(list) || list.length > 128) return fail();
    const seen = new Set<string>();
    for (const v of list) { const r = record(v, keys, optional); const key = identifier(r.id); if (seen.has(key)) fail(); seen.add(key); visit(r); }
  }
  if (m.palette !== undefined) {
    const palette = record(m.palette, ["commands", "settings_sections"]);
    const ids = new Set<string>();
    const checkRows = (value: unknown, check: (v: unknown) => string): void => {
      if (!Array.isArray(value) || value.length > 128) fail();
      for (const row of value) {
        const id = check(row);
        if (ids.has(id)) fail();
        ids.add(id);
      }
    };
    const label = (value: unknown): string => {
      const valueText = text(value);
      if (/\p{Cc}/u.test(valueText)) fail();
      return valueText;
    };
    const address = (value: unknown): void => {
      if (!/^\/(?!\/)[^\\]*$/.test(label(value))) fail();
    };
    checkRows(palette.commands, value => {
      const row = record(value, ["id", "label", "group"], ["words", "go", "op"]);
      label(row.label); label(row.group);
      if ((row.go === undefined) === (row.op === undefined)) fail();
      if (row.go !== undefined) address(row.go);
      if (row.op !== undefined) identifier(row.op);
      if (row.words !== undefined) {
        if (!Array.isArray(row.words) || row.words.length > 32) fail();
        row.words.forEach(label);
      }
      return identifier(row.id);
    });
    checkRows(palette.settings_sections, value => {
      const row = record(value, ["id", "label", "go"]);
      label(row.label); address(row.go);
      return identifier(row.id);
    });
  }
  const operations = new Map<string, Record<string, unknown>>();
  rows("kinds", ["id", "schema", "storage", "claimed_text"], r => {
    schema(r.schema); if (typeof r.claimed_text !== "boolean") fail();
    if (r.searchable !== undefined && (typeof r.searchable !== "boolean" || (r.searchable && !r.claimed_text))) fail();
    const live = typeof r.schema === "object" && r.schema !== null
      ? (r.schema as Record<string, unknown>)["x-calternal-live-editing"] : undefined;
    if (live !== undefined && (typeof live !== "boolean" || (live && !r.claimed_text))) fail();
    // Claimed text is Markdown; other kinds name a safe lowercase extension.
    if (r.extension === undefined ? !r.claimed_text : typeof r.extension !== "string" || !/^[a-z0-9]{1,16}$/.test(r.extension) || (r.claimed_text && r.extension !== "md")) fail();
    if (!r.storage || typeof r.storage !== "object" || Array.isArray(r.storage)) fail();
    const placement = r.storage as Record<string, unknown>;
    if (Object.hasOwn(placement, "folder")) {
      record(placement, ["folder"], ["subdirectories"]); const folder = text(placement.folder, 255);
      if (folder !== folder.trim() || folder.startsWith(".") || /[/\\:\p{Cc}]/u.test(folder) || folder.normalize("NFC") !== folder || folder.toLowerCase() === "trash") fail();
      if (placement.subdirectories !== undefined) {
        if (!Array.isArray(placement.subdirectories) || placement.subdirectories.length < 1 || placement.subdirectories.length > 8) fail();
        const components=[folder,...placement.subdirectories];
        if (reservedInstructionParts.every((part,index)=>typeof components[index] === "string" && (components[index] as string).normalize("NFC").toLowerCase() === part)) fail();
        for (const component of placement.subdirectories) {
          const name = text(component, 255);
          if (name !== name.trim() || name.startsWith(".") || /[/\\:\p{Cc}]/u.test(name) || name.normalize("NFC") !== name || name.toLowerCase() === "trash") fail();
        }
      }
    } else { record(placement, ["hidden"]); if (placement.hidden !== true) fail(); }
  }, ["extension", "searchable"]);
  const content = m.network === undefined
    ? undefined
    : record(m.network, [], ["content_fetch"]).content_fetch;
  if (content !== undefined) {
    const fetch = record(content, ["profile", "sources"]);
    if (fetch.profile !== "images_fonts" || !Array.isArray(fetch.sources)
        || fetch.sources.length === 0 || fetch.sources.length > 32) fail();
    const kinds = new Set(
      (m.kinds as { id: string; claimed_text: boolean }[] | undefined)
        ?.filter(kind => kind.claimed_text).map(kind => kind.id),
    );
    for (const value of fetch.sources) {
      const source = record(value, ["kind", "field"]);
      if (source.field !== "text" || !kinds.delete(identifier(source.kind))) fail();
    }
  }
  if (!Array.isArray(m.host_calls)
      || m.host_calls.includes("content_prepare_v1") !== (content !== undefined)) fail();
  rows("operations", ["id", "input_schema", "output_schema", "required_capability", "mutates", "idempotency", "risk"], r => {
    if (!["read", "reversible", "irreversible", "external", "widens_access"].includes(String(r.risk)) || (r.risk === "read") === r.mutates) fail();
    operations.set(identifier(r.id), r); schema(r.input_schema); schema(r.output_schema);
    if (!capabilities.has(String(r.required_capability)) || typeof r.mutates !== "boolean" || !["read_only", "request_id", "never"].includes(String(r.idempotency)) || r.mutates === (r.idempotency === "read_only") || (r.mutates && r.required_capability === "space.read")) fail();
    target(r.target, r.input_schema);
  }, ["target"]);
  const op = (r: Record<string, unknown>): void => { if (!operations.has(String(r.operation))) fail(); };
  rows("collections", ["id", "kind", "name"], r => { identifier(r.kind); text(r.name); });
  rows("views", ["id", "region", "component"], r => { identifier(r.component); if (!["sidebar", "main", "right_card"].includes(String(r.region))) fail(); });
  if (m.owned_slots != null) {
    if (!Array.isArray(m.owned_slots) || m.owned_slots.length > 32) fail();
    rows("owned_slots", ["id", "required_capability", "accepts", "max_contributions"], row => {
      const capability = row.required_capability;
      if (typeof capability !== "string" || !capabilities.has(capability) || capability.startsWith("spaces.")) fail();
      const { accepts, max_contributions: limit } = row;
      if (!Array.isArray(accepts) || !accepts.length || accepts.length > 2 ||
          new Set(accepts).size !== accepts.length || accepts.some(kind => kind !== "generated" && kind !== "command")) fail();
      if (typeof limit !== "number" || !Number.isInteger(limit) || limit < 1 || limit > 32) fail();
    });
  }
  if (m.slots != null) {
    if (!Array.isArray(m.slots) || m.slots.length > 32) fail();
    rows("slots", ["id", "label", "slot", "renderer"], row => {
      if (/\p{Cc}/u.test(text(row.label, 128))) fail();
      const slot = row.slot;
      const owned = typeof slot !== "string";
      if (owned) {
        const target = record(slot, ["plugin_id", "id"]);
        identifier(target.plugin_id);
        identifier(target.id);
      } else if (!["sidebar_section", "card_window", "composer_row", "message_decoration", "thread_edge", "settings_section", "palette_commands", "toasts"].includes(slot)) fail();
      const type = (row.renderer as { type?: unknown } | null)?.type;
      if (type === "sealed") {
        const renderer = record(row.renderer, ["type", "asset"]);
        const path = text(renderer.asset, 256);
        if (owned || ["palette_commands", "message_decoration", "toasts"].includes(String(slot)) || !path.endsWith(".html") || !path.split("/").every(part => id.test(part))) fail();
      } else {
        const renderer = record(row.renderer, ["type", "operation"]);
        const operation = operations.get(identifier(renderer.operation));
        if (!operation) fail();
        if (type === "command" && (owned || slot === "palette_commands")) return;
        if (type !== "generated" || slot === "palette_commands" || operation.mutates) fail();
        const output = operation.output_schema as { properties?: { view?: unknown }; required?: unknown } | null;
        const view = output?.properties?.view;
        if (!view || typeof view !== "object" || Array.isArray(view) || !Array.isArray(output?.required) || !output.required.includes("view")) fail();
      }
    });
  }
  rows("attention", ["id", "operation"], op);
  rows("settings", ["id", "value_type", "default"], r => {
    if (!["string", "boolean", "number"].includes(String(r.value_type)) || typeof r.default !== r.value_type || (typeof r.default === "number" && !Number.isFinite(r.default))) fail();
    if (r.scope != null && r.scope !== "space" && r.scope !== "personal") fail();
  }, ["scope"]);
  rows("schedules", ["id", "operation", "expression"], r => { op(r); text(r.expression); });
  rows("webhooks", ["id", "operation"], op);
  rows("hooks", ["id", "mode", "patterns", "operation"], r => { op(r); if (!["watch", "block"].includes(String(r.mode)) || !Array.isArray(r.patterns) || !r.patterns.length || r.patterns.length > 128) fail(); for (const p of r.patterns as unknown[]) identifier(typeof p === "string" && p.endsWith(".*") ? p.slice(0, -2) : p); });
  rows("themes", ["id", "stylesheet"], r => { const p = text(r.stylesheet, 4096); if (!p.endsWith(".css") || /[:\\]/.test(p) || p.split("/").some(x => !x || x === "." || x === "..")) fail(); });
  for (const key of ["secrets", "host_calls"]) { const list = m[key]; if (!Array.isArray(list) || list.length > 128 || new Set(list).size !== list.length) fail(); for (const v of list as unknown[]) { identifier(v); if (key === "host_calls" && !isHostCallKind(v)) fail(); } }
  return (Object.hasOwn(m, "kinds") ? m : { ...m, kinds: [] }) as ManifestV1;
}
