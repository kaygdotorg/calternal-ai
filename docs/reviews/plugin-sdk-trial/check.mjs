import { verifyPackage } from "@calternal/plugin-sdk/verify-package";

function* content(node) {
  if (!node) return;
  yield node;
  for (const child of node.children ?? []) yield* content(child);
}

function patch(side, value) {
  const action = side?.[0];
  return action?.fullSwipe === true && action.operation === "host.item.patch-metadata"
    && action.inputs?.item?.row === "id"
    && action.inputs?.field?.literal === "done_today"
    && action.inputs?.value?.literal === value;
}

function createsHabit(form, kind, operations) {
  if (form.type !== "form" || form.fields.length !== 2 || !form.fields.every(field => field.required)) return false;
  const operation = operations.find(operation => operation.id === form.submit?.operation);
  if (operation?.creates !== kind || !operation.methods.includes("item_save")) return false;
  const keys = ["title", "cadence"];
  const sources = keys.map(key => form.submit.inputs?.[key]);
  const fieldIds = new Set(form.fields.map(field => field.id));
  return fieldIds.size === 2 && new Set(sources.map(source => source?.field)).size === 2
    && keys.every((key, index) => {
      const source = sources[index];
      return source?.form === form.id && fieldIds.has(source.field)
        && operation.input_schema?.properties?.[key]?.type === "string"
        && operation.input_schema.required?.includes(key);
    });
}

function missingFeatures(template, kind, operations) {
  const nodes = [...content(template.view?.root)];
  const binds = template.binds ?? [];
  const habitLists = binds.filter(bind => bind.source.type === "list" && bind.source.kind === kind.id
    && nodes.some(node => node.type === "list" && node.bind === bind.id));
  const searchId = template.chrome?.footer?.search?.bind;
  return [
    [habitLists.length > 0, "bound habit content list"],
    [habitLists.some(bind => bind.states?.empty?.title?.trim()), "empty state on the habit list"],
    [habitLists.some(bind => {
      const { leadingActions, trailingActions } = bind.row ?? {};
      return (patch(leadingActions, true) && patch(trailingActions, false))
        || (patch(leadingActions, false) && patch(trailingActions, true));
    }), "opposite full swipes that set and clear done_today on the habit row"],
    [nodes.some(node => createsHabit(node, kind.id, operations)), "two-field add form wired to item creation"],
    [binds.some(bind => bind.id === searchId && bind.source.type === "search" && bind.source.kind === kind.id),
      "footer search over the same habit kind"],
  ].filter(([present]) => !present).map(([, label]) => label);
}

try {
  const { manifest, apps } = await verifyPackage(process.argv[2] ?? "dist");
  const kinds = manifest.kinds.filter(kind => kind.claimed_text && kind.searchable
    && kind.schema?.properties?.title?.type === "string"
    && kind.schema.properties.cadence?.type === "string"
    && kind.schema.properties.done_today?.type === "boolean"
    && kind.indexed_fields?.done_today?.type === "Boolean"
    && manifest.reads?.includes(kind.id));
  if (!kinds.length) throw new Error("Missing searchable habit kind with title, cadence and indexed done_today.");
  if (!apps.length) throw new Error("Missing app description.");
  const remaining = kinds.flatMap(kind => apps.map(({ template }) => missingFeatures(template, kind, manifest.operations)));
  const closest = remaining.reduce((best, candidate) => candidate.length < best.length ? candidate : best);
  if (closest.length) throw new Error("Missing " + closest.join(", ") + ".");
  console.log("PASS: emitted package declares all Habit Tracker features.");
} catch (error) {
  console.error("FAIL: " + error.message);
  process.exitCode = 1;
}
