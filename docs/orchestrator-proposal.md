# The orchestrator plugin: proposal

Status: proposal for kayg to grill, 2026-11-17. Nothing here is built. Terms are in the
[glossary](glossary.md). Earlier accepted decisions are kept and cited from
`architecture-proposal.md` section 8 and `feature-gap-assessment.md`; where today's
decisions changed one, this says so.

## The idea in one paragraph

The orchestrator is an ordinary plugin called **Threads**. A thread is a conversation
with an agent, stored as a Markdown file in a space. Sending a message starts a **turn**,
which is a job that runs an official agent program (Claude Code, Codex) in a sandbox on a
machine and streams its output back. A **subagent** is a child thread whose turn is a
child job. Everything else (review, automations, workflows, questions, approvals) is
built from those pieces. The plugin uses only the public plugin API, so anything it does,
another plugin or a user's own agent can do too.

## What exists already, and what each piece becomes

| Orchestrator concept | Built on | State |
| --- | --- | --- |
| Thread | An item of kind `threads`, a Markdown file in `AI/Chats/` | Storage built |
| Message | A block in the thread's file, with a stable ID | Block IDs designed, not built |
| Turn | A job with a stream and checkpoints | Built |
| Subagent | A child thread whose turn is a child job | Child jobs built |
| Where it runs | A machine and a sandbox, placed by the server | Built (containers) |
| Agent backend | A provider plugin that launches the official program | Not built |
| Question, approval, secret request | One "request" shown in the sticky bar | Not built |
| Automation | A Markdown file whose schedule starts a turn | Not built |
| Review | A collection of what a turn changed | Not built |

## Threads and messages

- A thread is one Markdown file per conversation, in the space's `AI/Chats/` folder, with
  its ID in frontmatter. Messages are blocks with stable IDs; tool calls are folded step
  blocks; plugin cards are fenced blocks. This is the inherited, accepted layout.
- The file is the content. A rebuildable index in appdata makes long threads fast: the app
  never parses a whole file to show the last screen.
- A thread's address is `/threads/<id>`; a message's is `/threads/<id>/<block-id>`. The
  list of a space's threads is a collection.
- Streaming text is not written to the file token by token. The file gets the message when
  a checkpoint is saved and when the turn ends. A reader who joins mid-turn gets the last
  checkpoint plus the live stream.
- Editing a sent message keeps the original in history and never reruns anything. Forking
  from any message creates a new thread file with `branched-from:`; a fork never replays
  past actions. (Accepted 2026-11-16.)

## Turns

A turn is one job. Its life:

1. **Admit.** `threads.send` records the message and creates the job before returning.
   The request ID makes a retry safe.
2. **Place.** The server picks a machine: the one the thread used last, else the person's
   chosen machine, else the pool if their group allows it. No capacity means a visible
   wait with a queue position.
3. **Prepare.** The machine starts or resumes the thread's sandbox and makes sure the
   workspace is there.
4. **Run.** The provider plugin launches the official program inside the sandbox, with the
   person's own login already present there, and passes the message and any context
   contributions.
5. **Stream.** Output flows back as chunks. Tool calls appear as steps. Checkpoints are
   saved at natural boundaries (after each tool call, at least every few seconds of text).
6. **End.** The final message is written to the thread file; the job is done, failed,
   cancelled or outcome-unknown. Hooks for "run ended" fire.

Steering: a message sent while a turn is running steers it by default, as in bb. Stop is
an operation. Changing model or provider applies to the next message. (Accepted.)

## Subagents

- A main thread starts a subagent through an operation, `threads.spawn`, that the agent
  calls as a tool. The child is a real thread: its own file, its own sandbox, its own
  turn job with the parent's job as parent.
- The child inherits only what the spawn names: a task, chosen context, and a workspace
  rule (share the parent's checkout read-only, or get its own worktree). It never inherits
  more access than the person who started the parent has.
- The parent can wait for children (built) and gets exactly one result from each. A child
  that needs the person raises a request; it shows in the bar of the parent's thread too.
- Finished children archive automatically; failed or blocked ones stay visible. (Accepted.)
- Ten subagents is ten child jobs. Limits come from the person's group and from machine
  capacity, not from a constant.

## Where work runs

- **Machine.** The person's own enrolled machine, or the pool. On their own machine, work
  waits for that machine and never moves by itself (accepted 2026-11-16). On the pool, a
  silent machine's work moves and the person is told (decided 2026-11-17).
- **Sandbox.** One per thread, inside the person's boundary on that machine. The exact
  boundary (a microVM per person, per space or per workspace) follows the hybrid
  measurement.
- **Login.** The person signs in to Claude Code or Codex once, inside their own sandbox
  home, through the provider's own flow. That home is a persistent volume belonging to the
  person, so the login survives new threads, restarts and frozen sandboxes. The server
  process never reads or stores it.
- **Workspace.** A workspace is a named source of files for a thread: a git repository, a
  folder on the person's machine, or files from the space. Coding threads get their own
  git worktree and branch by default; several helpers never write the same checkout.
  (Accepted.)
- **Files from the space.** Putting a space's files into a sandbox is a read, checked
  against the person's access at that moment. Writing back is an ordinary operation
  against the version that was copied, merged by the rules for its kind, never an
  overwrite. (Accepted.)

## Provider plugins

A provider is a plugin that knows how to run one official agent program. It declares:

- how to check the program is installed and signed in, and how to start its sign-in;
- how to start, resume, steer and stop a session;
- how to turn the program's output into the common stream: text, a tool step, a question,
  an approval request, a usage report, an error;
- which models and options it offers.

It runs on the machine, inside the sandbox, as a machine-side plugin operation. The first
two are Claude Code and Codex, each through its official interface (the same ones bb and
T3 Code use). A provider never handles the login itself.

When a provider's limit is hit, the turn waits with the reported reset time and resumes by
itself; no fallback unless the person configured one. (Accepted.)

## Requests: one thing that waits on you

A question from an agent, an approval, a secret request and "this plugin needs setup" are
one kind of thing: a **request**. Each has an ID, who it is for, what it is about, and
its possible answers.

- It shows in the sticky bar above the composer of the thread it belongs to, and in the
  attention card (at most three there).
- Approvals and secret entry are drawn by the app, never by a plugin, and an agent cannot
  answer them.
- Questions and approvals do not expire. Secret requests expire after 15 minutes and can
  be asked again.
- Answering is an operation, so it works from any device, the CLI and a notification.
- A turn waiting on a request is a job in the `waiting` state holding no process.

## Review

- Each turn records what it changed: files in the workspace (as a diff against where it
  started), items in the space (as operations with before and after), commits and pull
  requests.
- The right card shows this for the open thread: a list of changes, each opening its diff.
  Undo is per change where the kind supports it.
- For coding: the agent pushes a branch and opens a pull request under the person's own
  git credentials; checks are followed until ready for review; nothing merges on its own.
  A merged pull request's worktree and branch are cleaned up. (Accepted.)

## The three regions

| Region | Threads shows |
| --- | --- |
| Sidebar | The space's threads, grouped by workspace, children nested under parents, with running and waiting states |
| Middle card | The thread: messages, steps, cards, the request bar and the composer |
| Right card, bottom tabs | AI chat is the thread itself, so the tabs are: Changes, Run (machine, model, usage, state), Files |

From any other app, the right card's default tab is a thread about what is open there.

## What it takes from bb, and what it changes

Kept: threads with child threads, steering a running turn, worktrees per task, skills,
automations, plugins that add tools, a CLI for everything.

Changed:

- **Several people.** Threads live in spaces with roles. A Manager can cancel a turn;
  members who can read the space can watch it.
- **Threads are files.** One Markdown file per chat, readable without the app.
- **Cloud machines.** No machine of your own is needed; the pool runs agents in sandboxes.
- **Sandboxed by default.** An agent runs in a sandbox even on your own machine, unless
  you choose direct access to a folder.
- **One request bar.** Nothing an agent needs from you scrolls away.
- **Apps are tools.** Notes, tasks and the rest are operations an agent calls with your
  access, not screens it has to drive.

## Simplifications this depends on

| Idea | Status |
| --- | --- |
| One way to run untrusted code: the sandbox (plugin workers too) | Agreed in principle; being measured |
| One kind of long-running work: the job | Agreed |
| One thing that waits on you: the request | Agreed |
| Everything is an item | Agreed |
| Plugins all the way down | Agreed |

## Build order

1. Provider plugin for Claude Code, run by hand from the CLI: send, stream, stop.
2. `threads` kind and file format; `threads.send` creating a turn job on a machine.
3. Requests: questions and approvals through the bar's operations (no UI yet).
4. Subagents: `threads.spawn`, wait, one result.
5. Workspaces: git worktree per thread; changes recorded for review.
6. Codex provider. Automations. Then the UI.

Each step is usable from the CLI before any UI exists, so development can move onto it
early.

## Questions for kayg

1. **Cals.** The accepted design has cals (a saved personality with abilities and memory)
   as optional. Build threads first with plain provider and model, and add cals after?
2. **Thread file growth.** A week-long thread with ten subagents can be very large. One
   file per thread as accepted, with long tool output kept outside the file and linked?
3. **Sandbox on your own machine.** Sandboxed by default with an explicit "direct folder"
   choice, or direct by default as bb does today?
4. **Who sees a thread.** In a shared space, is a new thread visible to every member who
   can read the space, or private to its starter until shared?
5. **Subagent workspaces.** Default for a coding subagent: its own worktree, or the
   parent's checkout read-only?
6. **What from bb do you most want kept exactly, and what annoys you most?**
