// Writes data/fixtures.json: what a host would deliver to one viewer (Ana) for the spike's screens, with every value already
// filled in as her: names resolved, times as instants, her own unread counts and mutes, quotes resolved with her rights, the
// redacted record emptied. No server; nothing here is real. Run: node data/make-fixtures.mjs
import { writeFileSync } from "node:fs";

const P = { ana: { id: "p-ana", name: "Ana Silva" }, ivo: { id: "p-ivo", name: "Ivo Novak" }, hana: { id: "p-hana", name: "Hana Sato" }, lee: { id: "p-lee", name: "Lee Park" } };
const at = (iso) => ({ at: iso });
const people = (...ps) => ({ people: ps.map((p) => ({ ...p, ...(p === P.ana ? { me: true } : {}) })) });

// Reading List: one window per sidebar collection. Values are keyed by the look's field names.
const reading = [
  ["r1", "The unreasonable effectiveness of plain text", "brandur.org", "2026-11-20T08:12:00Z", false],
  ["r2", "SwiftUI scroll position, explained", "developer.apple.com", "2026-11-19T21:40:00Z", false],
  ["r3", "Local-first software: you own your data", "inkandswitch.com", "2026-11-19T10:05:00Z", false],
  ["r4", "How SQLite scales read concurrency", "fly.io", "2026-11-18T16:30:00Z", true],
  ["r5", "A field guide to calm interfaces", "calmtech.com", "2026-11-17T09:00:00Z", false],
  ["r6", "Designing for the thumb zone", "smashingmagazine.com", "2026-11-15T13:20:00Z", true],
  ["r7", "Why we sync with CRDTs (and when we don't)", "linear.app", "2026-11-12T19:45:00Z", false],
  ["r8", "Typography for long reading on phones", "practicaltypography.com", "2026-11-10T07:30:00Z", true],
].map(([item, title, site, created, read]) => ({ item, kind: "reading-list/reading", values: { title, "site(url)": site, created: at(created), read } }));

const chat = (item, title, ps, excerpt, last, unread, muted = false) =>
  ({ item, kind: "calternal-chat/chats", values: { title, people: people(...ps), "last.excerpt": excerpt, "last.at": at(last), "me.unread": unread, "me.muted": muted } });
const chats = {
  c1: chat("c1", "Release 2.4", [P.ana, P.ivo, P.hana], "Hana: @Ana can you post it?", "2026-11-20T10:15:00Z", 3),
  c2: chat("c2", "", [P.ana, P.hana], "See you at nine.", "2026-11-19T19:02:00Z", 0),
  c3: chat("c3", "Lisbon crew", [P.ana, P.ivo, P.hana, P.lee], "Lee: the flat has a balcony", "2026-11-20T09:58:00Z", 12, true),
  c4: chat("c4", "", [P.ana, P.ivo], "Can you review my branch?", "2026-11-20T08:20:00Z", 1),
  c5: chat("c5", "Sync design with Claude", [P.ana], "Claude: here is the plan for the outbox", "2026-11-18T22:10:00Z", 0),
  c6: chat("c6", "Book club", [P.ana, P.lee, P.hana], "Next one: Piranesi", "2026-11-16T18:00:00Z", 0),
  c7: chat("c7", "Old flat", [P.ana, P.lee], "Keys returned.", "2026-09-30T12:00:00Z", 0),
  c8: chat("c8", "Conference 2025", [P.ana, P.ivo, P.lee], "Slides are up", "2025-10-04T15:00:00Z", 0),
};

// The chat transcript of c1. `person` is who the record is for; `run` is set when an agent wrote it for that person.
const claude = { name: "Claude", icon: "claude" };
const R = [
  [1, P.ivo, "2026-11-19T09:02:00Z", "Morning! Did the dependency audit finish overnight?"],
  [2, P.ivo, "2026-11-19T09:03:00Z", "I would like to ship 2.4 today if it is clean."],
  [3, P.ana, "2026-11-19T09:10:00Z", "It finished at six. Three advisories, all in dev dependencies, none reachable from the app."],
  [4, P.hana, "2026-11-19T09:12:00Z", "Can someone summarise them for the release notes?"],
  [5, P.ana, "2026-11-19T09:14:00Z", "On it: I will ask Claude to write them up and check `npm audit` again.", { quote: "a04" }],
  [6, P.ana, "2026-11-19T09:16:00Z", "All three are in `eslint-plugin-x` and its test helpers. The fixed version is **4.1.2**; after the update `npm audit --omit=dev` finds no vulnerabilities.", { quote: "a05" }, { run: claude }],
  [7, P.hana, "2026-11-19T09:20:00Z", "Great, thank you [@Ana](person:p-ana). That goes straight into the notes.", { mentions: ["p-ana"] }, { edited: true }],
  [8, P.ivo, "2026-11-19T10:05:00Z", "Build 2.4.0-rc1 is up on staging."],
  [9, P.ivo, "2026-11-19T10:06:00Z", "Smoke tests pass on the web; iOS is still running."],
  [10, P.ana, "2026-11-19T10:30:00Z", "iOS passes too. One flaky test in the share sheet, filed as #212."],
  [11, P.hana, "2026-11-19T11:02:00Z", "The release notes draft is in Notes."],
  [12, P.ivo, "2026-11-19T11:40:00Z", "", {}, { redacted: true }],
  [13, P.ivo, "2026-11-19T11:41:00Z", "Sorry, wrong chat."],
  [14, P.ana, "2026-11-19T14:15:00Z", "Shall we cut the release tomorrow morning?"],
  [15, P.hana, "2026-11-19T14:20:00Z", "Yes, nine works for me."],
  [16, P.ivo, "2026-11-19T14:22:00Z", "Works for me too."],
  [17, P.ivo, "2026-11-20T08:51:00Z", "Good morning. Tagging 2.4.0 now."],
  [18, P.ivo, "2026-11-20T08:52:00Z", "The tag is pushed and the pipeline is running."],
  [19, P.ana, "2026-11-20T08:58:00Z", "Watching it."],
  [20, P.hana, "2026-11-20T09:05:00Z", "Did I miss something yesterday?", { quote: "a12" }],
  [21, P.ivo, "2026-11-20T09:06:00Z", "No, that one was meant for the infra chat.", { quote: "a20" }],
  [22, P.ana, "2026-11-20T09:40:00Z", "The pipeline is green. Publishing to the stores."],
  [23, P.ana, "2026-11-20T10:02:00Z", "Claude, could you draft the website changelog from the release notes?"],
  [24, P.ana, "2026-11-20T10:04:00Z", "Here is a draft:\n**2.4** brings faster sync, a new share sheet and three fixes in development tools. The full notes are in the app.", { quote: "a23" }, { run: claude }],
  [25, P.hana, "2026-11-20T10:10:00Z", "Looks good. I would add the iPad keyboard shortcuts."],
  [26, P.ivo, "2026-11-20T10:12:00Z", "Agreed, and the store listing is live."],
  [27, P.hana, "2026-11-20T10:15:00Z", "[@Ana](person:p-ana) can you post it?", { mentions: ["p-ana"] }],
];
const anchor = (n) => `a${String(n).padStart(2, "0")}`;
const recs = R.map(([n, person, when, body, fields = {}, extra = {}]) => {
  const { run, ...flags } = extra;
  return { anchor: anchor(n), body, fields: { mentions: [], ...fields }, stamp: { person, ...(run ? { run } : {}), at: when }, ...flags };
});
// A quote is resolved as the viewer: the quoted record's author and an excerpt, or `gone` when it was redacted.
const refsFor = (page) => Object.fromEntries(page.flatMap((r) => (r.fields.quote ? [r.fields.quote] : [])).map((a) => {
  const q = recs.find((r) => r.anchor === a);
  return [a, q.redacted ? { gone: true } : { author: q.stamp.run ? q.stamp.run.name : q.stamp.person.name, excerpt: q.body.split("\n")[0].slice(0, 140) }];
}));
const PAGE = 16;
const newest = recs.slice(-PAGE), older = recs.slice(0, -PAGE);

const fixtures = {
  viewer: { ...P.ana, tz: "Europe/Lisbon", now: "2026-11-20T10:20:00Z" },
  slots: {
    "calternal-chat/composer": "calternal-chat-composer",
    "calternal-chat-ai/harness": [{ plugin: "calternal-chat-claude-code", name: "Claude", icon: "claude" }, { plugin: "calternal-chat-codex", name: "Codex", icon: "codex" }],
  },
  windows: {
    "reading-list/Unread": { rows: reading.filter((r) => !r.values.read), count: 5 },
    "reading-list/All": { rows: reading, count: 8 },
    "calternal-chat/Pinned": { rows: [chats.c1, chats.c2, chats.c3, chats.c4] },
    "calternal-chat/Chats": { rows: [chats.c1, chats.c3, chats.c4, chats.c2, chats.c5, chats.c6] },
    "calternal-chat/Archived": { rows: [chats.c7, chats.c8] },
  },
  records: {
    c1: { position: "a24", unread: 3, page: { records: newest, refs: refsFor(newest), before: "older" },
      pages: { older: { records: older, refs: refsFor(older), before: null } } },
  },
};
writeFileSync(new URL("./fixtures.json", import.meta.url), JSON.stringify(fixtures, null, 1) + "\n");
console.log("wrote data/fixtures.json");
