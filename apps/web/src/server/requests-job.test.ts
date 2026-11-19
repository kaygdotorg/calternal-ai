// A plugin job's wait as the server lists it (a request with a job and no thread) becomes a request with `jobId`, which the Inbox lists on its own.
import { expect, it } from "vitest";
import { jobPromptsOf } from "../apps/threads/inbox";
import { requestOf, type WireRequest } from "./requests";

const q: WireRequest = { id: "r1", space_id: "s", thread_id: null, job_id: "j1", detail: { kind: "question", label: "Release selection", form: { genui_version: 2, source: "Form([], \"x\")" } } };
const a: WireRequest = { id: "r2", space_id: "s", thread_id: null, job_id: "j2", detail: { kind: "approval", app: "jobs-demo", write: false, lifetimes: ["once"], action: { label: "Waiting for a release" } } as never };

it("keeps the job and lists both as plugin prompts", () => {
  const rs = [q, a].flatMap((w) => requestOf(w) ?? []);
  expect(rs.map((r) => [r.kind, r.jobId, r.threadId])).toEqual([["question", "j1", ""], ["approval", "j2", ""]]);
  expect(jobPromptsOf(rs).map((p) => [p.reason, p.title])).toEqual([["Question", "Release selection"], ["Approval", "The agent asks for read access to jobs-demo"]]);
});
