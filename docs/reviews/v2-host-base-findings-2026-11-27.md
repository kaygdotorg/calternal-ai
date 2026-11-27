# Revision 9 and the Chat fixtures · 2026-11-27

The declaration files target the format engine and host from the Chat-first
plan. The TypeScript shows operation signatures and calls for slices 3 and
6. The existing SDK and Claude Code adapter remain on their current
contracts until those slices wire the new route.

## Decided by the lane

- **Chat §2.3, §2.4 and §2.20.** A chat has a stable item ID and a
  title-derived folder under `Chats/` with `chat.md` inside. The host
  handles path and link changes when the coded `rename` writes the new
  title. The template starts empty so the first text in the body comes
  from a host-stamped message. This follows old ADR 0056 for identity and
  links, and the worked Chat example for storage.
- **Chat §2.8–2.9, §4.3 and §5.5.** Send writes one complete record, with
  the message in its Markdown body as in the host spike, and optional
  quote, mentions and attachment links as declared fields. Attachments refer to
  stored items and use the existing file ingestion rules (old ADR 0028).
  Pins, mute, draft and read position are personal. The pinned collection
  is a compact view with manual order. Chats lists all unarchived chats,
  including a new personal chat with an empty people field. Groups and
  People are filter examples. Sharing and notifications come after the
  first Chat build.
- **Chat §7.4 and §8.1.** Chat fills its own composer with `send`. The
  Claude Code harness fills the harness place with `start`. The harness's
  kind supplies the operation declaration. Host run state and logs
  supply the live run's identity and status. A completed reply takes the
  same Chat append route, under the harness's declared call and the
  run's scope and Stop fence. This avoids a second plugin-owned log.
- **Format §2.6, §4.7 and §12.** Each read line is an allow-list with OR
  within the line and AND across applicable lines. A named role is a
  minimum. Folder rules inherit and intersect with kind rules. Filter
  before projection, ordering, pagination and aggregation. This keeps
  read and change authority in the existing check.
- **Format §5.3.** The default in personal kind collections combines
  local and shared same-kind rows. `shared with me` and `ignoring shared`
  narrow that source. Deduplicate by source identity. Changes use the
  source's ordinary change right, and collection shares do not propagate
  through incoming rows. Whole-kind formula traversals stay local.
- **Format §2.6a, §7.7 and §12.** Reuse `fills: receive` for share
  targets. Progress is a numeric field of the active operation's item.
  The receiving person controls locked notification actions, as in the
  sixth set. First-party bridges remain app-owned. Space icons are
  checked assets and the accent uses the existing colour type.
- **Format §7.9 and §8.1.** Refusals use bracketed operation keys, ordered
  checks and the authorized inputs of the checked operation. Rules
  cannot gain an event or read grants. Session scope uses the host's
  quiet-hour boundary and an explicit target set. Wider scope values
  have no contract in the Chat-first plan and are left out of its
  fixtures.

## What the fixtures cannot express

- §2.9 and §4.3 can compare a record's timestamp with a saved time. They
  cannot count records strictly after an anchor in ledger order, so
  records sharing the read timestamp cannot be distinguished by this
  unread formula. The fixture counts all later timestamps, regardless
  of author. It does not implement the old app's edit-as-fork behavior.
- §7.4 names a place and its operation, but has no typed place payload,
  initial prompt binding or gesture-to-operation sequencing grammar.
  The composer needs the current draft and chat ID. For the first run,
  the caller commits the prompt with `send` before dispatching the
  harness place with the chat ID. The adapter uses the scoped record
  stream. Selecting an exact prompting record on a busy transcript and
  submitting send-and-start as one retryable gesture need a host
  contract. A slot label alone supplies neither.
- §8.1 declares program IDs, but has no mapping from an ID to an
  adapter executable or protocol. Slice 6 wires
  `calternal-provider-claude-code` from the existing adapter, including
  streamed log output, the completed reply call and the host-stamped
  run ID. The manifest does not grant sign-in access. The format also
  has no generic way to recover an existing run handle from its ID or
  to declare a Harnesses collection over host run logs.
- §8.1 does not spell out the SDK export names, frame encoding for
  record writes, or operation input schemas. The two `operations.ts`
  files sketch these against the named calls. They depend on the SDK
  and host lanes for runtime validation and generated help.
- §7.9 needs a typed operation-input schema at install time to validate
  its conditions. On-device evaluation needs the same authorized inputs.
  The compiler and host must settle how missing input schemas and
  evaluation errors are surfaced before implementing this declaration.
  Chat declares no refusals.

## Review gate

Reviewed the inherited format diff and compare points against the format,
the build plan, the named branch tips and the older ADRs via git. Changes
add no runtime, storage writer, shared type copy or deployment dependency.
There were no builds, parser tests, VM runs or hot-path measurements in
this design lane. The build plan assigns parser acceptance to slice 0 and
the two-flush append measurement to slice 2.

## Simplification opportunities

- Let the host's run identity and log drive run views before adding a
  plugin-maintained run index.
- Give typed operation inputs one declaration that serves calls, places,
  refusal compilation and generated help.
