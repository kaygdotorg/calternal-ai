/**
 * Flood mode of the fixtures build (owner, 2026-11-19): `?flood` (5,000 threads and 5,000 notes) or `?flood=N`, to try lists and panes at a size no
 * person has yet, with realistic variety. `?churn` (4 changes a second) or `?churn=N` adds a live stream of changes: a thread gets a message (its state, position and
 * time change), a new thread arrives at the top, one is renamed or removed; the same for notes. Fixtures only: the real client never reads them.
 * The parameters are documented in `apps/web/e2e/README.md` ("Flood mode").
 */
import { rng } from "../apps/threads/generate";

/** What the address asks for: the number of threads and of notes, and changes per second (0 when no churn). Null without `?flood`. */
export function floodOf(): { count: number; churn: number } | null {
  let q: URLSearchParams;
  try { q = new URLSearchParams(location.search); } catch { return null; }
  if (!q.has("flood")) return null;
  const count = Math.min(20000, Math.max(1, Number(q.get("flood")) || 5000));
  const churn = q.has("churn") ? Math.min(50, Math.max(1, Number(q.get("churn")) || 4)) : 0;
  return { count, churn };
}

/** A stable 32-bit hash of an id (the fixtures' per-thread seed for generated threads: the character-sum seed of the small fixtures repeats across thousands of ids). */
export function seedOf(id: string): number {
  let h = 2166136261;
  for (let i = 0; i < id.length; i++) { h ^= id.charCodeAt(i); h = Math.imul(h, 16777619); }
  return h >>> 0;
}

const WORDS = ["journal", "replay", "invite", "passkey", "storage", "tier", "mesh", "receipt", "sandbox", "provider", "catalog", "validator", "backup", "restore", "migration", "conformance", "latency", "cache", "index", "sidebar", "composer", "canvas", "wallpaper", "contrast", "keyboard", "viewport", "scheduler", "retry", "snapshot", "lease", "quota", "thread", "space", "plugin", "adapter", "bridge", "release", "changelog", "benchmark", "profile", "flake", "timeout", "rollback", "audit", "token", "session", "daemon", "socket", "manifest", "schema"];
const VERBS = ["Fix", "Split", "Rename", "Review", "Plan", "Investigate", "Draft", "Speed up", "Add a test for", "Remove", "Document", "Port", "Reproduce", "Bisect", "Clean up", "Wire up", "Benchmark", "Explain", "Migrate", "Harden"];
const TAILS = ["", "", "", " on dev-02", " after restart", " (flaky)", " for the Mac app", " before Friday", ": what changed since the last run, which suites are affected, and what the owner needs to decide before it can be merged"];

/** A realistic title: short, medium, or a long one that has to ellipsize. */
export function floodTitle(r: () => number): string {
  const w = (): string => WORDS[Math.floor(r() * WORDS.length)]!;
  const t = `${VERBS[Math.floor(r() * VERBS.length)]!} the ${w()} ${w()}${TAILS[Math.floor(r() * TAILS.length)]!}`;
  return r() < 0.15 ? t.split(" ").slice(0, 3).join(" ") : t;
}

/** A seeded generator for the flood (one stream, so churn is repeatable). */
export const floodRng = (seed: number): (() => number) => rng(seed);
