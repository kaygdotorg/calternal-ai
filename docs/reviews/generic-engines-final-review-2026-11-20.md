SEAL WITH LISTED FOLLOW-UPS

**1. My 19 earlier findings, checked against 5abca642**
1. Reattach after restart: CLOSED. §A.2 now freezes the attempt at renewal +5 s (or supervisor heartbeat +2 s) and tears it down at +10 min. Thaw needs a fresh one-use renewal. §A.3 needs proof of the same boot, container, attempt and fence. C.1 tests 30 s and 5 min outages. In practice, a run waiting on a provider stream usually needs Resume after a long outage, but a running tool or build continues. That is no worse than today, where any machine silence interrupts.
2. Threads as a host app: CLOSED. §A.6 replaces every sealed Turns surface. The orchestrator declares `threads` itself with `conversation: true`, any kind may opt in, `ProviderId` is open, and archive, unarchive, snooze and stop are generic `host.item.*` and `host.execution.stop`.
3. Canonical file: CLOSED. The Markdown file is canonical, every projection rebuilds from it, and the host writes it (interim, Q3).
4. Token path: CLOSED. The adapter emits the envelope, the host commits it, and the `execution` wait wakes the plugin only on phase, request or terminal transitions.
5. Composer commands: CLOSED (§A.1).
6. Agent discovery: CLOSED as an owner question (Q1). The interim is a labelled regression; see the owner-question section.
7. Steer and child: CLOSED. `execution_input`, and `execution_spawn_child` with tree reservation, intersected grants and a generic parent edge. Small gaps are in follow-ups 4 and 5.
8. Harness qualification: CLOSED. Any package may qualify a digest, and a labelled personal-machine fallback exists. Follow-up 6 asks who runs the suite.
9. Thumbnail flicker: CLOSED. Last authorized image kept as stale, any editor whose state matches may capture, and the device paints locally.
10. Declarative extraction: CLOSED. Closed selectors run at save with no wake. Follow-up 7 asks for tag-selector neutrality.
11. Tags and labels: CLOSED for tags (`TextSet`, `contains`, total key budget). Labels: DEFERRED WITH REASON (Photos).
12. Original file I/O: DEFERRED WITH REASON (§B.4, no first-release consumer).
13. Similarity custody: DEFERRED WITH REASON (§B.4, Q4).
14. Indexer authority: DEFERRED WITH REASON (Q2). The sealed creator interim applies, and a Manager can take over the schedule only.
15. Scratch placement: CLOSED (heavy VM only; activated with its first consumer).
16. Spool cap: CLOSED (counts unconsumed records).
17. Member row cache: DEFERRED WITH REASON. Revision 13.2 stays because of body retention after revocation; the change is offered separately to the owner. Cost: a cold offline launch paints title-only rows, and the snippet and image fill in after a fresh reply.
18. Reversible cutover: CLOSED. Cutover at zero attempts (wait, not fence), full render diff with projections deleted, and a rollback target that understands the new representation.
19. Spike pass conditions: CLOSED. C.1 has falsifiable numbers for Send, outage, publication, cutover, tags and previews.

**2. Paper exercises (proposal + sealed contract + revision 13.3)**

(a) Orchestrator as an ordinary plugin: it can be built.
- Thread: it declares its own `threads` kind.
- Send: host composer, then the declared Send. Starting is committed cold, then `execution_start`.
- Provider, model and machine choice: provider contributions plus host-validated placement.
- Streaming: adapter envelope, written by the host.
- Approval: host Inbox and a one-use gateway grant.
- Stop, archive with Undo, snooze: `host.execution.stop` and `host.item.*`.
- Restart: freeze plus proof-based reattach; otherwise the person resumes.
- Titles and automations: package jobs and durable timers.

Where I still slow down:
- Steer: §A.1 does not say whether a message sent during a live attempt reaches `execution_input` without waking the plugin VM (follow-up 4).
- Child agent: who creates the child conversation item when the agent asks for one is unstated (follow-up 5).
- §A.5 says the orchestrator "owns tool-step presentation", but §A.3 puts the adapter and host renderer in charge of tool summaries. The orchestrator can only shape them through its declarations.
- Approvals: Codex's actionable native approvals may become dismissal-only (follow-up 2).
- Shipped logins: the orchestrator ships only once both shipped adapters qualify under the credential rule (follow-up 1).

(b) Community runner with another provider: it has no privilege gap. It can declare its own conversation kind and gets transcript, Inbox `waiting_on`, search, archive, snooze, stop, child runs and an open provider mark. It can declare a provider route and qualify its own digest. What it cannot do that the first-party runner can:
- appear in the orchestrator's Threads list. That is ownership (interim Q6), and symmetric: it can instead contribute a provider to the orchestrator;
- while unqualified, show a display or approval route. §A.2 says an unqualified harness gets "no qualified host display", which leaves unclear what such a run can show at all (follow-up 6).

The first-party adapters face exactly the same gate.

(c) Notes as an ordinary plugin: it can be built.
- Tags: the declared extractor with `TextSet` and `contains`, published with the body commit and no wake.
- Rows: revision 13.3's second line and leading row image, with portrait binding from §B.1.
- Thumbnails: the device snapshot with grant, stale-keep and local paint. Without an image, the miniature comes from the summary; a blank one covers old or empty notes.
- Search: revision 13's all-field search.
- Live rooms and cold create: room concurrency stays in the host; revision 13's `host_create` gives the cold create.
- Rename: link rewrite as a Notes job.

I got stuck nowhere. Two notes:
- Notes Trash retention (`crates/server/src/notes/retention.rs`) is not in the §A.5 table. It must become generic item lifecycle (follow-up 8).
- A rename leaves links broken until the rewrite job runs. That is accepted, since the owner accepts the space waking for such work.

New major gaps: none

None of the following meets the major bar:
- No contradiction with an owner decision. The device-taken thumbnail follows the later "File over app" entry, which supersedes rule (3) of "No special privileges".
- Nothing app-specific stays in the host except generic engines: the conversation file, Inbox, extractors and the sign-in notice.
- Nothing needed while asleep or for security moved out of the host.
- No first-release mechanism serves only Photos.
- No thread can be lost or stranded. Unproven sessions lose native Resume only; the thread stays readable and continuable.

The closest to major is follow-up 1. If subscription logins cannot be brokered, the orchestrator cannot use Claude Code or Codex under the rule. §A.5 gates the cutover on qualification, though, so the old engine keeps working and nothing regresses; the risk is schedule, not design.

**Follow-ups to carry into the build**

1. **Credential brokering, prove first (§A.2).** Neither the adapter nor the harness may read credentials, yet today the person runs `claude auth login` inside their sandbox and the official CLI reads its own login (decision record: "each person signs in to the official programs inside their own sandbox").
   - Before other build work, show that Claude Code (OAuth subscription) and Codex (ChatGPT login) run with an opaque handle through the supervisor-owned transport.
   - Specify how a sign-in flow delivers its result into the transport without the adapter seeing it. §A.5 says adapters "own login behavior", which is unexplained under this rule.
   - Say explicitly that the old engine keeps today's behaviour until both adapters qualify.
2. **Codex approvals (§A.4).** Under "executable material the host and gateway can verify", Codex's actionable on-request native approvals become dismissal-only. Codex in Approve for me would stall on its own in-sandbox prompts, which is a visible regression.
   - Keep native approvals actionable when their effect stays inside the attempt's sandbox, bound to the live process and request ID.
   - Make dismissal-only the rule only for requests that would cross the sandbox (today's host rule).
3. **Legacy sessions (§A.5).** Today's record holds (account, thread, machine) and no profile, so in practice every migrated thread loses native Resume.
   - Production was reinstalled and has no threads yet, so the cost is small now but grows if production is used before cutover.
   - Either accept the per-account, per-machine provider home as the historical profile when it has not been signed in again since, or record the profile in today's binding now.
4. **Steer (§A.1).** Let the Send policy declare "while an attempt is live, forward to `execution_input`" so the host delivers a steer without a plugin wake. Otherwise each steer costs a wake (0.3–0.5 s p95, about 2.5 s at 20 concurrent wakes).
5. **Child conversation (§A.2/§A.6).** State who creates the child conversation item when an agent requests a child. The likely answer is a package job through generic create. Also state whether that wakes the plugin VM.
6. **Qualification (§A.2).** State who triggers and runs the qualification suite for a community digest on a hosted installation; it should be automatic, with no first-party sign-off. Define what display an unqualified personal-machine run gets.
7. **Tag selector (§B.1).** Make the inline-tag selector parametric: declared marker, hierarchy separator and exclusions, with no Notes defaults in host code. This satisfies review gate 9 and the generic-engine rule.
8. **§A.5 table.** Add Notes Trash retention as generic item lifecycle. Also change "tool-step presentation" to mean declared rendering of committed summaries.
9. **Contract revision (§C.3).** Write it as a delta on revision 13.3. Bind portrait aspect to the existing leading row image and reuse its second line; add no second image mechanism.
10. **Send in a sleeping space.** The 8 s ready gate is looser than today's roughly 2–6 s. Report the time to first token beside it, and treat a p95 above today's as a finding even when the gate passes.

**Owner questions (section C)**
- All six are the right questions. Each stands on its own with an example and a recommendation. Q2, Q4 and Q5 are correctly marked as not blocking.
- **Q1:** its interim (refs only) is safe, but it is a visible regression from what ships today. In Full Access and Approve for me, "find my notes about Lisbon" works today, including through the agent's CLI, and stops working under the interim. Mark Q1 as blocking the orchestrator cutover, not only the catalogue freeze, so the regression never ships by default.
- **Q3 and Q6:** the interims are safe and change nothing visible today.
- **The cache option:** fine as written.
- **Add Q7** (it comes out of follow-up 1): "If a provider's own login cannot be brokered, may its official program read its own login inside a single person's sandbox on that person's own machine, as today?" A prompt-injected agent could take the person's provider token, but otherwise that provider is unavailable. Interim: disabled, with the old engine kept until the owner answers.