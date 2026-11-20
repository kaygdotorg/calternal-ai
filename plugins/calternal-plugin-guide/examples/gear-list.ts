import { defineApp, defineChrome, defineKind, defineOperation, definePlugin, defineView } from "@calternal/plugin-sdk";

// Kind and indexed fields
const kind = defineKind({
  id: "gear",
  icon: "book",
  storage: { hidden: true },
  claimed_text: true,
  searchable: true,
  schema: {
    type: "object",
    additionalProperties: false,
    required: ["title", "location"],
    properties: {
      title: { type: "string" },
      location: { type: "string" },
      packed: { type: "boolean" },
    },
  },
  indexed_fields: { packed: { type: "Boolean", audience: "member_item" } },
});

// Operation declaration
const addGear = defineOperation({
  id: "gear.add",
  title: "Add gear",
  creates: kind.id,
  methods: ["item_create"],
  required_capability: "content.write",
  risk: "reversible",
  idempotency: "request_id",
  input_schema: {
    type: "object",
    additionalProperties: false,
    required: ["title", "location"],
    properties: { title: { type: "string" }, location: { type: "string" } },
  },
  output_schema: {
    type: "object",
    additionalProperties: false,
    required: ["created"],
    properties: { created: { $ref: "host-catalogue-v2.json#/$defs/ItemRef" } },
  },
});

// List Bind, empty state and row swipes
const view = defineView({
  binds: [{
    id: "gear",
    source: { type: "list", kind: kind.id },
    fields: ["packed"],
    states: { empty: { title: "No gear yet" } },
    row: {
      trailingActions: [{
        id: "pack",
        title: "Mark packed",
        operation: "host.item.patch-metadata",
        fullSwipe: true,
        inputs: { item: { row: "id" }, field: { literal: "packed" }, value: { literal: true } },
      }],
      leadingActions: [{
        id: "unpack",
        title: "Clear packed",
        operation: "host.item.patch-metadata",
        fullSwipe: true,
        inputs: { item: { row: "id" }, field: { literal: "packed" }, value: { literal: false } },
      }],
    },
  }],
  root: {
    type: "stack",
    children: [
      { type: "list", bind: "gear" },
      // Form fields and submit Invoke
      {
        type: "form",
        id: "add",
        fields: [
          { id: "title", label: "Name", input: "text", required: true },
          { id: "location", label: "Location", input: "text", required: true },
        ],
        submit: {
          id: "add",
          title: "Add gear",
          operation: addGear.id,
          inputs: { title: { form: "add", field: "title" }, location: { form: "add", field: "location" } },
          then: { route: { name: "item" } },
        },
      },
    ],
  },
});

// Search Bind and footer
const chrome = defineChrome({
  binds: [{ id: "gear-search", source: { type: "search", kind: kind.id } }],
  footer: { search: { bind: "gear-search", placeholder: "Search gear" } },
});

// App and plugin assembly
export default definePlugin({
  api_version: 2,
  id: "gear-list",
  name: "Gear List",
  version: "1.0.0",
  icon: "book",
  kinds: [kind],
  operations: [addGear],
  apps: [defineApp({ id: "gear", name: "Gear", icon: "book", view, chrome })],
});
