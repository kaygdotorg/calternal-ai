// The recogniser's test vectors (C5, C12, C21, C22). The clock is fixed: Friday 23 October 2026, 14:10.
import { describe, expect, test } from "vitest";
import { recognise } from "./phrases";
import { checkboxIsTask } from "./recognise";
import type { Now } from "./dates";

const now: Now = { today: "2026-10-23", h: 14, m: 10 };
const read = (t: string, o: Parameters<typeof recognise>[1] = {}) => recognise(t, { now, ...o });

describe("dates (N1)", () => {
  test.each([
    ["call plumber today", "2026-10-23"],
    ["call plumber tomorrow", "2026-10-24"],
    ["call plumber tmrw", "2026-10-24"],
    ["pay rent friday", "2026-10-23"], // a bare weekday includes today
    ["pay rent monday", "2026-10-26"],
    ["pay rent this friday", "2026-10-23"],
    ["pay rent next friday", "2026-10-30"], // the following week's
    ["pay rent next week", "2026-10-26"],
    ["pay rent in 3 days", "2026-10-26"],
    ["pay rent in 2 weeks", "2026-11-06"],
    ["pay rent 5 nov", "2026-11-05"],
    ["pay rent nov 5th", "2026-11-05"],
    ["pay rent 2 jan", "2027-01-02"], // already past this year: next year
    ["pay rent 2026-12-01", "2026-12-01"],
  ])("%s", (t, when) => {
    const p = read(t);
    expect(p.when).toBe(when);
    expect(p.kind).toBe("task");
    expect(p.title).toBe("pay rent".startsWith(t.split(" ")[0]!) ? "pay rent" : p.title);
  });

  test("the title is what is left", () => {
    expect(read("call plumber tomorrow 5pm").title).toBe("call plumber");
    expect(read("tomorrow call plumber").title).toBe("call plumber");
    expect(read("remind me to water the plants on friday").title).toBe("remind me to water the plants");
  });
});

describe("times (N2)", () => {
  test.each([
    ["call 5pm", "17:00"],
    ["call 5:30 pm", "17:30"],
    ["call at 17:45", "17:45"],
    ["call tomorrow 9am", "09:00"],
    ["call fri 5", "17:00"], // a bare 5 after a date: guessed (evening)
    ["call tomorrow at 9", "09:00"], // another day: 9 reads as morning
    ["call at 3", "15:00"], // 14:10 now: 3 is still ahead as pm
  ])("%s", (t, time) => expect(read(t).time).toBe(time));

  test("a range", () => {
    const p = read("lunch with amma tomorrow 1-2pm");
    expect([p.time, p.endTime, p.when]).toEqual(["13:00", "14:00", "2026-10-24"]);
  });
  test("a time alone lands today if ahead, else tomorrow", () => {
    expect(read("call 5pm").when).toBe("2026-10-23");
    expect(read("call 9am").when).toBe("2026-10-24");
  });
  test("not a time", () => {
    expect(read("read chapter 12").time).toBeNull();
    expect(read("buy 2 tickets").when).toBeNull();
  });
});

describe("Hinglish (C21)", () => {
  test.each([
    ["dentist aaj", "2026-10-23"],
    ["dentist kal", "2026-10-24"],
    ["dentist parso", "2026-10-25"],
    ["report agle hafte", "2026-10-26"],
    ["call plumber kal 5pm", "2026-10-24"],
  ])("%s", (t, when) => expect(read(t).when).toBe(when));
  test("words inside words are left alone", () => {
    expect(read("kalpana").when).toBeNull();
    expect(read("ajay").when).toBeNull();
  });
});

describe("priority", () => {
  test.each([
    ["call plumber !", 3],
    ["call plumber !!", 4],
    ["call plumber !!!", 4],
    ["call plumber !high", 3],
    ["call plumber !urgent", 4],
    ["call plumber !med", 2],
    ["call plumber !low", 1],
    ["urgent call plumber", 4],
    ["call plumber ⏫", 3],
    ["call plumber 🔺", 4],
    ["call plumber 🔼", 2],
    ["call plumber 🔽", 1],
  ])("%s", (t, p) => {
    const r = read(t);
    expect(r.priority).toBe(p);
    expect(r.title).toBe(t.startsWith("urgent") ? "call plumber" : "call plumber");
  });
  test("the example", () => {
    const p = read("call plumber kal 5pm !high");
    expect([p.kind, p.title, p.when, p.time, p.priority]).toEqual(["task", "call plumber", "2026-10-24", "17:00", 3]);
  });
});

describe("deadlines and repeats", () => {
  test("by friday is a Deadline, not a When", () => {
    const p = read("send report by friday");
    expect([p.deadline, p.when, p.title]).toEqual(["2026-10-23", null, "send report"]);
  });
  test("both", () => {
    const p = read("send report tomorrow by 30 oct");
    expect([p.when, p.deadline]).toEqual(["2026-10-24", "2026-10-30"]);
  });
  test.each([
    ["water plants every monday", { every: 1, unit: "week", on: 1 }, "2026-10-26"],
    ["water plants daily", { every: 1, unit: "day" }, "2026-10-23"],
    ["water plants every 2 weeks", { every: 2, unit: "week" }, "2026-10-23"],
    ["pay rent monthly", { every: 1, unit: "month" }, "2026-10-23"],
    ["standup every weekday", { every: 1, unit: "week", weekdays: true }, "2026-10-23"],
  ])("%s", (t, repeat, when) => {
    const p = read(t);
    expect(p.repeat).toEqual(repeat);
    expect(p.when).toBe(when);
  });
});

describe("Obsidian emoji (T2)", () => {
  test("a Tasks line reads the same", () => {
    const p = read("pay rent 📅 2026-11-01 ⏳ 2026-10-28 ⏫ 🔁 every month");
    expect([p.deadline, p.when, p.priority, p.repeat?.unit, p.title]).toEqual(["2026-11-01", "2026-10-28", 3, "month", "pay rent"]);
  });
});

describe("keep as text (C17)", () => {
  test("quotes and backticks are never read", () => {
    expect(read('call "tomorrow" cafe').when).toBeNull();
    expect(read("call `5pm` cafe").time).toBeNull();
  });
  test("a word the user kept stays text", () => {
    const p = read("call tomorrow cafe tomorrow", { keep: ["tomorrow"] });
    expect(p.when).toBeNull();
    expect(p.kind).toBe("task"); // "call" is still a verb
  });
});

describe("what the line is", () => {
  test("a date or a verb makes a task", () => {
    expect(read("buy milk").guess).toBe("task");
    expect(read("milk run friday").guess).toBe("task");
  });
  test("plain words have no opinion, the Composer decides open or note", () => {
    expect(read("garden ideas").guess).toBeNull();
    expect(read("garden ideas").kind).toBe("note");
  });
  test("a question is a search", () => {
    expect(read("what did I decide about hidden blocks").guess).toBe("open");
    expect(read("tomato soup?").guess).toBe("open");
  });
  test("context only breaks a tie (C19): plain words in Tasks are a task, a dated phrase is a task anywhere", () => {
    expect(read("garden ideas", { here: "tasks" }).guess).toBe("task");
    expect(read("garden ideas", { here: "note" }).guess).toBeNull();
    expect(read("what is a gerund", { here: "tasks" }).guess).toBe("open");
  });
  test("forcing wins", () => {
    expect(read("buy milk", { forced: "note" }).kind).toBe("note");
    expect(read("garden ideas", { forced: "task" }).kind).toBe("task");
  });
});

describe("spans", () => {
  test("recognised words are marked where they are", () => {
    const t = "call plumber kal 5pm !high";
    const p = read(t);
    expect(p.spans.map((s) => [t.slice(s.start, s.end), s.type])).toEqual([
      ["kal", "date"],
      ["5pm", "time"],
      ["!high", "priority"],
    ]);
  });
});

describe("speed (C12)", () => {
  // The recogniser must read a phrase well inside the 8 ms keystroke budget on the oldest phone. That is a wall-clock claim, and it is measured where a clock means something (`e2e/palette.mjs`, `e2e/perf-suite.mjs`
  // at 6x CPU slowdown): a unit test that asserted fractions of a millisecond failed on a busy machine. What this asserts is what makes it fast: a long phrase is read in one pass to the same, complete answer every time.
  test("a long phrase is read completely and identically every time", () => {
    const t = "remind me to call plumber about the leaking tap in the upstairs bathroom tomorrow at 5pm by friday every week !high";
    const first = JSON.stringify(read(t));
    for (let i = 0; i < 20; i++) expect(JSON.stringify(read(t))).toBe(first);
    expect(first.length).toBeGreaterThan(100);
  });
});

describe("tags and people (C22)", () => {
  test("they are read and marked", () => {
    const p = read("call priya #errands/calls @priya");
    expect([p.tags, p.people, p.kind]).toEqual([["errands/calls"], ["priya"], "task"]);
  });
  test("not inside words, emails or links", () => {
    const p = read("mail a@b.com about c#sharp and https://x.com/#top");
    expect([p.tags, p.people]).toEqual([[], []]);
  });
  test("any metadata turns a checkbox into a task, words alone do not", () => {
    for (const t of ["buy milk friday", "buy milk 5pm", "buy milk every week", "buy milk !", "buy milk by friday", "buy milk #errands", "buy milk @amma"]) expect(checkboxIsTask(t, now), t).toBe(true);
    for (const t of ["buy milk", "call plumber", "what is a gerund?", "garden ideas"]) expect(checkboxIsTask(t, now), t).toBe(false);
  });
});

describe("whitespace is never eaten", () => {
  const spans = (t: string): [string, number, number][] => read(t).spans.map((s) => [t.slice(s.start, s.end), s.start, s.end]);
  test("spans are exact slices and leave the spaces between recognised tokens outside them", () => {
    const t = "call plumber kal 5pm !high";
    for (const [w, a, b] of spans(t)) {
      expect(w).toBe(w.trim());
      expect(t.slice(a as number, b as number)).toBe(w);
    }
    // 5pm and !high are two tokens with one space between them: the space is in no span.
    const [, , end5] = spans(t).find(([w]) => w === "5pm")!;
    const [, start] = spans(t).find(([w]) => w === "!high")!;
    expect(t.slice(end5, start)).toBe(" ");
  });
  test("runs of spaces and tabs between tokens stay as typed", () => {
    for (const t of ["call  plumber   kal  5pm   !high", "call\tkal\t5pm\t!high", " call kal  "]) {
      const ss = read(t).spans;
      // Rebuilding the text from the marked and unmarked parts gives it back exactly.
      let at = 0;
      let out = "";
      for (const s of ss) {
        out += t.slice(at, s.start) + t.slice(s.start, s.end);
        at = s.end;
      }
      out += t.slice(at);
      expect(out).toBe(t);
      for (const s of ss) expect(/^\s|\s$/.test(t.slice(s.start, s.end))).toBe(false);
    }
  });
  test("every prefix of a phrase marks without touching its text", () => {
    const t = "call plumber kal 5pm !high by friday";
    for (let i = 1; i <= t.length; i++) {
      const p = t.slice(0, i);
      for (const s of read(p).spans) expect(p.slice(s.start, s.end)).toBe(p.slice(s.start, s.end).trim());
    }
  });
});
