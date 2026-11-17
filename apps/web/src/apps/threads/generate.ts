/**
 * Deterministic fixture messages. `messageAt(thread, index)` builds message number `index` from a seeded generator, so a thread of
 * 10,000 messages costs no memory until a range is read, and the same index always gives the same message (the performance script
 * depends on that). Fixture-only: nothing here exists in the real client.
 */
import type { Block, Message, Step, StepKind } from "./model";

/** A small seeded generator (mulberry32). */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = <T>(r: () => number, xs: readonly T[]): T => xs[Math.floor(r() * xs.length)]!;

const ASKS = [
  "Can you check why the journal replay is slow after a restart?",
  "Add a test for the expired invite case.",
  "Rename `Space::open` to `Space::attach` everywhere and update the docs.",
  "The CLI prints the table twice when `--json` is off. Find out why.",
  "Look at the failing conformance run and tell me what broke.",
  "Draft the migration note for the storage layout change.",
  "Can we cache the catalog validators instead of building them per call?",
  "Why does the passkey sign-in flake on the dev VM?",
  "Split `store.rs`, it is past 600 lines.",
  "Make the plugin worker report its exit reason.",
];
const OPENERS = [
  "I looked at it and the cause is in the way the journal is read back.",
  "Found it. The validator is rebuilt for every request.",
  "That is a small change. Here is what I did.",
  "Two things are going on, and only one of them is a bug.",
  "The test was asserting the old behaviour, so I updated it with the fix.",
  "I read the three call sites first; none of them need the lock held across the write.",
];
const BODIES = [
  "The replay reads each entry with its own `fsync` check, which is what dominates after a restart. Reading the tail once and verifying the hash chain in memory brings it from 4.1 s to 0.3 s on the dev VM (a development-host number, not a benchmark).",
  "`catalog()` built the schema map on every call. It is constant, so it now lives in a `OnceLock` and the request path only clones a reference.",
  "The invite expiry was compared in local time in one place and UTC in another. Both now use the shared time helper, and the new test pins the boundary at the exact second.",
  "I did not touch the public API. The doc comments changed with the code, and `scripts/check-docs.sh` passes.",
  "The flake is the passkey ceremony timing out on a cold VM: the first request pays the TLS handshake. A warm-up request before the ceremony fixes it without changing the product code.",
];
const FOLLOW = [
  "Want me to open the pull request now, or look at the other call sites first?",
  "I left the old name in the changelog on purpose, since it records history.",
  "Nothing else needed changing. The full suite is green on the dev VM.",
  "One open question: should the cache be per space or per installation? I assumed per installation because the catalog never varies.",
];
const LISTS = [
  "- the reader no longer calls `sync_all` per entry\n- the hash chain is verified once over the tail\n- the bookmark is written after the last entry, not before",
  "1. build the schema once\n2. share it behind an `Arc`\n3. drop the per-call allocation",
];
const CODE = [
  "```rust\nlet tail = journal.read_tail(max_bytes)?;\nverify_chain(&tail)?;\nOk(tail.into_entries())\n```",
  "```ts\nconst compiled = compile(source);\nif (!compiled.program) throw new Error(formatDiagnostics(compiled.diagnostics));\n```",
  "```sh\nnix develop -c cargo test -j6 -p calternal-store\n```",
];
// A picture, to have media under the floating bars (a generated SVG as a data URI; nothing is fetched).
const PICTURE = "![A chart of the replay time before and after](data:image/svg+xml;utf8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='640' height='260'%3E%3Cdefs%3E%3ClinearGradient id='g' x1='0' y1='0' x2='1' y2='1'%3E%3Cstop offset='0' stop-color='%23ffd9a0'/%3E%3Cstop offset='1' stop-color='%23101820'/%3E%3C/linearGradient%3E%3C/defs%3E%3Crect width='640' height='260' fill='url(%23g)'/%3E%3Crect x='60' y='70' width='90' height='150' fill='%23ffffff'/%3E%3Crect x='200' y='170' width='90' height='50' fill='%23ffffff'/%3E%3C/svg%3E)";
const CMDS: Record<StepKind, string[]> = {
  read: ["crates/store/src/journal.rs", "crates/auth/src/invite.rs", "docs/addresses.md", "crates/cli/src/main.rs"],
  edit: ["crates/store/src/journal.rs", "crates/store/src/lib.rs", "crates/auth/tests/invite.rs"],
  run: ["cargo test -j6 -p calternal-store", "cargo fmt --check", "scripts/check-docs.sh", "git diff --stat"],
  search: ["rg -n 'sync_all' crates", "rg -n 'catalog\\(' crates/genui", "rg -n 'OnceLock' crates"],
  spawn: ["review the storage change"],
};
const TITLE: Record<StepKind, string> = { read: "Read", edit: "Edited", run: "Ran", search: "Searched", spawn: "Started a helper" };

/** A long log: `lines` lines of cargo-like output, the kind of tool result that makes a thread slow if drawn naively. */
export function longOutput(r: () => number, lines: number): string {
  const out: string[] = [];
  for (let i = 0; i < lines; i++) {
    const k = r();
    out.push(
      k < 0.6
        ? `   Compiling calternal-${pick(r, ["store", "auth", "fsio", "notes", "genui", "cli"])} v0.1.${i % 40} (/home/ubuntu/Developer/lanes/x/crates/${i % 9})`
        : k < 0.9
          ? `test store::journal::tests::case_${i} ... ok`
          : `warning: unused variable: \`entry_${i}\` --> crates/store/src/journal.rs:${100 + (i % 400)}:${5 + (i % 30)}`,
    );
  }
  return out.join("\n");
}

function steps(r: () => number, id: string, n: number, big: boolean): Step[] {
  const kinds: StepKind[] = ["read", "search", "read", "edit", "run", "run"];
  return Array.from({ length: n }, (_, i) => {
    const kind = pick(r, kinds);
    const last = i === n - 1;
    return {
      id: `${id}-s${i}`,
      kind,
      title: TITLE[kind],
      detail: pick(r, CMDS[kind]),
      output: kind === "run" ? (big && last ? longOutput(r, 2400) : longOutput(r, 3 + Math.floor(r() * 9))) : undefined,
      status: "ok" as const,
      ms: 120 + Math.floor(r() * 4000),
    };
  });
}

const TABLE_CARD = `Card("Test results", [Table(["Suite","Result","Time"],[["calternal-store",Badge("passed","good"),"4.1 s"],["calternal-auth",Badge("passed","good"),"2.7 s"],["calternal-cli",Badge("1 failed","bad"),"6.3 s"]]), Progress(2, 3, "Suites passed", "good", "Two of three suites passed")])`;

/** Message `index` of a thread with the given seed. Even indices are the person, odd are the agent (starting with the person). */
export function messageAt(threadId: string, seed: number, index: number, baseTime: number): Message {
  const r = rng(seed * 100003 + index * 7919 + 17);
  const at = baseTime + index * 90_000;
  const id = `${threadId}-m${index}`;
  if (index % 2 === 0) return { id, role: "user", at, blocks: [{ type: "text", text: pick(r, ASKS) }] };
  const blocks: Block[] = [{ type: "text", text: pick(r, OPENERS) }];
  if (r() < 0.8) blocks.push({ type: "steps", steps: steps(r, id, 3 + Math.floor(r() * 7), index % 250 === 1), ms: 4000 + Math.floor(r() * 90000) });
  blocks.push({ type: "text", text: [pick(r, BODIES), r() < 0.4 ? pick(r, LISTS) : "", r() < 0.4 ? pick(r, CODE) : "", r() < 0.12 ? PICTURE : ""].filter(Boolean).join("\n\n") });
  if (r() < 0.12) blocks.push({ type: "card", source: TABLE_CARD });
  blocks.push({ type: "text", text: pick(r, FOLLOW) });
  return { id, role: "agent", at, blocks };
}
