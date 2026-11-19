import {
  defineApp, defineChrome, defineKind, defineOperation, definePlugin, defineView,
} from "@calternal/plugin-sdk";

const item = "reading-item";
const reminder = "reading-reminder";
const title = { type: "string", minLength: 1, maxLength: 128 };
const timestamp = { type: "string", format: "date-time" };
const emptyOutput = { type: "object", additionalProperties: false, properties: {} };

export default definePlugin({
  api_version: 2,
  id: "calternal-reading-list",
  name: "Reading List",
  version: "1.0.0",
  icon: "book",
  kinds: [
    defineKind({
      id: item,
      icon: "book",
      storage: { hidden: true },
      claimed_text: true,
      searchable: true,
      host_create: { defaults: { title: "Untitled" } },
      schema: {
        type: "object",
        additionalProperties: false,
        required: ["title"],
        properties: {
          title,
          added: timestamp,
          read: { type: "boolean" },
          url: { type: "string", minLength: 1, maxLength: 4096 },
        },
      },
      indexed_fields: {
        added: { type: "Timestamp", audience: "member_item" },
        read: { type: "Boolean", audience: "member_item" },
      },
    }),
    // The reminder flow is scoped to the person's own space. The person
    // predicate below filters the list and grants no access.
    defineKind({
      id: reminder,
      icon: "clock",
      storage: { hidden: true },
      claimed_text: true,
      schema: {
        type: "object",
        additionalProperties: false,
        required: ["title"],
        properties: { title, due: timestamp, person: { type: "string", minLength: 1, maxLength: 128 } },
      },
      indexed_fields: {
        due: { type: "Timestamp", audience: "member_item" },
        person: { type: "AccountRef", audience: "viewer_self" },
      },
    }),
  ],
  operations: [
    defineOperation({
      id: "reading.add",
      title: "Add to Reading List",
      input_schema: {
        type: "object",
        additionalProperties: false,
        required: ["title", "url"],
        properties: { title, url: { type: "string", format: "uri", maxLength: 4096 } },
      },
      output_schema: { ...emptyOutput, required: [] },
      required_capability: "content.write",
      risk: "reversible",
      idempotency: "request_id",
      methods: ["item_save"],
      creates: item,
    }),
    defineOperation({
      id: "reading.remind",
      title: "Remind me",
      input_schema: {
        type: "object",
        additionalProperties: false,
        required: ["item", "when"],
        properties: {
          item: { type: "string", minLength: 1, maxLength: 128 },
          when: timestamp,
          note: { type: "string", minLength: 1, maxLength: 4096 },
        },
      },
      output_schema: { ...emptyOutput, required: [] },
      required_capability: "content.write",
      risk: "reversible",
      idempotency: "request_id",
      methods: ["schedule_create", "item_save"],
      target: { kind: item, field: "item" },
      creates: reminder,
    }),
    defineOperation({
      id: "reading.reminder-due",
      title: "Deliver reading reminder",
      input_schema: {
        type: "object",
        additionalProperties: false,
        required: ["reminder"],
        properties: { reminder: { type: "string", minLength: 1, maxLength: 128 } },
      },
      output_schema: emptyOutput,
      required_capability: "space.read",
      risk: "read",
      idempotency: "read_only",
      methods: ["item_head"],
      target: { kind: reminder, field: "reminder" },
    }),
  ],
  schedule_eligibility: [{ id: "reminder-due", operation: "reading.reminder-due" }],
  apps: [defineApp({
    id: "reading",
    name: "Reading List",
    icon: "book",
    view: defineView({
      title: "Reading list",
      binds: [
        {
          id: "items",
          source: { type: "list", kind: item },
          fields: ["added", "read"],
          sort: [{ field: "added", direction: "desc" }],
          row: {
            marks: [{
              kind: "icon",
              icon: "check",
              tone: "good",
              when: { field: "read", op: "eq", value: { literal: true } },
            }],
            trailingActions: [{
              id: "mark-read",
              title: "Mark read",
              icon: "check",
              operation: "host.item.patch-metadata",
              inputs: { item: { row: "id" }, field: { literal: "read" }, value: { literal: true } },
              tone: "good",
              fullSwipe: true,
            }],
          },
          states: { empty: { title: "Nothing to read", detail: "Add a link below." } },
        },
        {
          id: "reminders",
          source: { type: "list", kind: reminder },
          fields: ["due", "person"],
          filter: { all: [{ field: "person", op: "eq", value: { viewer: "person" } }] },
          sort: [{ field: "due", direction: "asc" }],
          window: { size: 20 },
          row: { marks: [{ kind: "text", field: "due", tone: "warn" }] },
          states: { empty: { title: "No reminders" } },
        },
      ],
      root: {
        type: "stack",
        title: "Reading list",
        children: [
          {
            type: "button",
            invoke: {
              id: "create-blank",
              title: "New blank item",
              operation: "host.item.create",
              inputs: { kind: { literal: item } },
              then: { route: { name: "item" } },
            },
          },
          {
            type: "form",
            id: "add",
            title: "Add a link",
            fields: [
              { id: "title", label: "Title", input: "text", required: true, maxLength: 120 },
              { id: "url", label: "Address", input: "url", required: true, maxLength: 500 },
            ],
            submit: {
              id: "add",
              title: "Add",
              operation: "reading.add",
              inputs: { title: { form: "add", field: "title" }, url: { form: "add", field: "url" } },
              then: { route: { name: "item" } },
            },
          },
          { type: "list", bind: "items", selectable: true, title: "To read" },
          {
            type: "form",
            id: "remind",
            title: "Remind me",
            fields: [
              { id: "when", label: "When", input: "date", required: true },
              { id: "note", label: "Note", input: "text", maxLength: 120 },
            ],
            submit: {
              id: "remind",
              title: "Set reminder",
              operation: "reading.remind",
              inputs: { item: { selection: "items" }, when: { form: "remind", field: "when" }, note: { form: "remind", field: "note" } },
            },
          },
          { type: "list", bind: "reminders", title: "Reminders" },
        ],
      },
    }),
    chrome: defineChrome({
      binds: [{
        id: "reading-search",
        source: { type: "search", kind: item },
        fields: ["read"],
        row: { icon: false },
      }],
      sidebar: { open: true, title: "Reading list", sections: [{ id: "items", title: "To read", bind: "items" }] },
      header: { title: "Reading list" },
      footer: { search: { bind: "reading-search", placeholder: "Search reading list" } },
    }),
  })],
});
