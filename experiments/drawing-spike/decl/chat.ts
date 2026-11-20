// Chat's core plugin, as authoring.md section 2.3 writes it (on lane/v2-host-base, 2026-11-21). The handlers are kept so the
// manifest is the author's whole file; sdk.ts replaces each with "code" when it is serialised. Lines that differ from the
// document end in a `// Dn` comment naming the change in docs/reviews/drawing-spike-2026-11-21.md.
import { plugin, kind, text, list, person, bool, link, collection, operation, slot } from "./sdk.ts";
const EDIT_HOURS = 48;
export default plugin({
  id: "calternal-chat", name: "Chat", version: "2.0.0", icon: "chat", grant: { people: true },
  kinds: { chats: kind({
    fields: { title: text().index(), people: list(person()).index(), archived: bool(false).index() },
    records: { fields: { mentions: list(person()).index(), quote: link() }, edit: "author", redact: "author-or-manager", position: "read", segment: "monthly" }, // D6
    folder: "Chats",
    row: { title: ["title", "people"], leading: { avatar: "people" }, detail: ["last.excerpt"], trailing: ["last.at", { count: "me.unread", quiet: "me.muted" }] }, // D11
    record: { body: "body", quote: "quote" }, // D6
    page: { records: { composer: "composer", actions: {
      reply: { compose: { quote: "record" }, place: "menu" },                 // fills the composer, sends nothing
      edit: { op: "calternal-chat/edit", inputs: { chat: "this", anchor: "record" }, defaults: { text: "record.body" }, when: "own" }, // D7
      delete: { op: "calternal-chat/delete", inputs: { chat: "this", anchor: "record" }, when: "own", confirm: "Delete for everyone?" },
      ask: { op: "calternal-chat-ai/call", inputs: { chat: "this", range: "records.selection" }, pick: { harness: "calternal-chat-ai/harness" } }, // D8
    } } },
  }) },
  slots: { composer: slot({ effect: "viewer", recommended: "calternal-chat-composer", gives: ["this", "draft", "quote"], may: ["calternal-chat/send"] }) },
  operations: {
    create: operation({ title: "New chat", input: { title: text(), people: list(person()) },
      async handler({ title, people }, ctx) {
        const chat = await ctx.files.create("chats", { fields: { title, people: [ctx.me.id, ...people] }, request: ctx.request });
        if (people.length) await ctx.people.share(chat, { people, role: "editor" });
        return { created: chat };
      } }),
    send: operation({ title: "Send", public: true, input: { chat: link(), text: text(), quote: link().optional() }, // D6
      async handler({ chat, text, quote }, ctx) {
        const { fields } = await ctx.files.read(chat);
        const mentions = fields.people.filter((p: PersonRef) => text.includes(`@${p}`));
        const { anchor } = await ctx.records.append(chat, { body: text, fields: { mentions, quote }, request: ctx.request }); // D6
        await ctx.state.set(`read.${chat}`, anchor);
        return { anchor };
      } }),
    reply: operation({ title: "Agent reply", public: true, input: { chat: link(), call: text(), text: text() },
      // Called by a run from its log (running section 2.7), once per log record, so ctx.request is unique per reply.
      handler: ({ chat, call, text }, ctx) =>
        ctx.records.append(chat, { body: text, fields: { quote: call }, request: ctx.request }) }), // D6
    edit: operation({ title: "Edit", input: { chat: link(), anchor: text(), text: text() },
      async handler({ chat, anchor, text }, ctx) {
        const [rec] = await ctx.records.read(chat, { anchors: [anchor] });
        if (Date.now() - Date.parse(rec.stamp.at) > EDIT_HOURS * 3_600_000) throw new Error("Messages can be edited for 48 hours.");
        return ctx.records.edit(chat, anchor, { body: text, expected: rec.revision });   // the host checks the author
      } }),
    delete: operation({ title: "Delete for everyone", input: { chat: link(), anchor: text() },
      handler: async ({ chat, anchor }, ctx) => (await ctx.records.redact(chat, anchor), {}) }),
    mute: operation({ title: "Mute", input: { chat: link(), on: bool() },
      handler: async ({ chat, on }, ctx) => (await ctx.state.set(`muted.${chat}`, on), {}) }),
  },
  sidebar: [collection("Pinned", { hand: true, view: "strip" }),
    collection("Chats", { kind: "chats", where: "!archived", sort: "-updated" }),
    collection("Archived", { kind: "chats", where: "archived", sort: "-updated" })],
});
