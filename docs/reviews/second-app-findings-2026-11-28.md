# Reading List and Tasks on the personal-space host · 2026-11-28

The same host installs Chat, Reading List and Tasks into one personal
space and serves them from the same checked catalogue. The two new
bundles contain Markdown declarations only. The preview also installs
Chat's existing harness. The verification record is on
[Forgejo #124](https://git.kayg.org/kayg/calternal-ai/issues/124).

## Reading List

- Section 16's URL and title add form, `domain(url)` site field, unread
  filter, date sort, read board, sidebar and search fields all work from
  declarations. The host supplies the addition date and the initial
  unread state. The named mark-read gesture checks the row and updates it
  in the same versioned write. The row disappears from Unread, and All
  reports the gesture unavailable. Built-in change and remove use the
  same Markdown and Trash paths as other editable kinds.
- The host needed URL and date validation at the JSON boundary, a way to
  invoke item assignments, and search across declared stored fields,
  computed fields and the Markdown body. The URL is stored as supplied.
  Domain evaluation and searching are local and perform no URL fetch.
- Section 16's reminder needs both a delivery template and the timed
  handler. This bundle uses the section's code-free variant. The format
  can name a notification and its template, while a timed, purely
  declarative delivery rule still needs a way to bind the due field,
  recipient and template. Host admission refuses notification templates
  while delivery is unavailable. Share targets and fetching page titles
  are outside the proof.

## Tasks

- Appendix B.2 supplies the task status, civil schedule and due date,
  priority, tags, recurrence, and row operations. Open filters on the
  computed status. All adds a list and a choice board, groups by status,
  and reports a count before pagination. Completing a task sets status
  and completion time together. Schedule uses a typed `ask` input. The
  unchanged recurrence engine returns the next occurrence from the
  scheduled slot or the completion time. This proof projects that value
  on the same item, leaving repeat rollover and completion history
  pending.
- The host needed civil `when` and RRULE input validation, atomic
  multi-field assignments, group and total projection, and live refresh
  when the host day changes. Reads evaluate against the current UTC
  date. Open subscriptions refresh on the next maintenance tick, which
  normally runs within 30 seconds of UTC midnight. Grouped repeat fields
  require the item's current version on change, since this host
  serializes writes and checks staleness at whole-item granularity.
- The full Appendix B.2 needs code for remove, skip and track and claims
  checkboxes in Notes and Calendar. The standalone bundle uses the
  ordinary Trash operation and omits those coded actions, cross-app
  fields and formulas, the heading mirror, the prompt-to-fields
  composer, duplication, and the completion log and streak. A completion
  timestamp supplies the boundary for completion-based recurrence.
  The format can name linked checkbox marks and calculate a next date.
  It has no one-line operation for atomically advancing a series,
  recording that occurrence, and reopening or creating the next task.
  Host admission refuses line claims, repeat window expansion, timers,
  conditional built-ins outside the supported assignment forms, and
  other declarations requiring unavailable runtime data or authority.
  Neither bundle uses `open where`.

Both bundles use revision 10 icon and label declarations for collections
and sidebar entries, with collection icons as the fallback. These rows
have no numeric field selected for badge display. The Tasks count is a
collection total, while Chat retains its numeric unread badge.

## Generic browser client handoff

- The app strip and sidebar already pick up the installed apps. The
  sidebar still needs to draw the entry icon with collection fallback.
  The icon catalogue needs the `book` token from section 16. The
  collection view currently draws every view through a single list with
  its own title, avatar and edit-time layout. Draw the declared columns
  including computed values. Add view selection and simple boards driven
  by the Boolean or choice field, with lane labels and order from its
  schema. The grouped collection exposes per-row `groups` and aggregate
  `groups` on the query and live response. `totals` follows the
  collection declaration order, and `namedTotals` uses declared names.
- The item window currently always mounts the record view. Render the
  checked item `page`, including the default field list and Markdown
  body. Wire generic change and Trash operations. The catalogue supplies
  implicit built-ins, the declared `search` field names, and `together`
  for fields that require submitting the current row version.
- The form already has a URL input and a multiline editor. It needs
  types for date, time, civil `when`, choice, Boolean, lists of tags,
  and RRULE editing. Use the catalogue's `choices` and `itemType` as
  the source of form values. Omit empty optional add inputs so the host
  can supply defaults. The present type strings do
  not distinguish tags from other lists of text, and there is no
  declared recurrence editor vocabulary. A basic typed list and a
  validated RRULE input can start with the catalogue as it stands.
- Send a named item operation through `s.write` with
  `{"changes":[{"op":"perform","item":"…","operation":"…","fields":{}}]}`.
  Use `when` to show availability and the declared placements for swipe,
  primary and menu actions. Build inputs from assignment entries marked
  `ask`. The host owns literal and computed assignment values. The
  current client sends a single literal item assignment as a raw
  change and has no branch for the multi-field Done operation. Chat's
  existing viewer state continues to use `s.state`.
- Send a collection or kind query with an optional `search` string. The
  host matches a case-insensitive substring in the kind's declared
  searchable values and body. The existing client has no search binding
  for this path. Plain field and board labels come from the declarations.
  Custom labels for individual choice values, the localized empty-board
  copy, and default field order beyond the order supplied by the host
  are presentation choices the format does not express here.

## Decisions and morning questions

- **Civil clock.** The standalone host has no space-zone setting. A task
  due on Friday can become overdue on different instants in UTC and
  the owner's zone. Use UTC until a space owns a zone. Setting this from
  the last viewer would make two viewers see different results for the
  same task. A stored zone gives one answer and adds zone validation and
  daylight-saving behavior to the host.
- **Cross-note writes.** A checkbox in a note could toggle an item and
  thereby touch copies of that box elsewhere. Keep line claims refused
  until the host can prove authority on the note and item in both
  directions and publish their changes atomically. Read-only line
  projection could land separately and would leave linked checkboxes
  visibly read-only during that stage.
- **Repeat writes.** A task completed before its next due date can keep
  its completion trail in one item or produce a separate occurrence.
  Keep next-occurrence projection until this lifecycle is decided. One
  row avoids accumulating items, while distinct occurrences make past
  completions and individual overrides directly addressable.
- **Preview backfill.** The first authenticated session-status check or
  sign-in after deployment installs missing operator-bundled apps. This
  covers existing browser sessions and leaves existing plugin bundles
  and items in place.

## Review gate

The host keeps the existing UUID scope, permission check, query budget,
Markdown writer, retry receipts, stage/commit/publication path and
per-space SQLite connection. The implementation uses the existing dependencies, tables and
retention policy. Synchronization stays with the per-space writer. The catalogue comes
from compiled definitions and is cached until install. A normal query
over either new kind reads in-memory rows and issues no item-file reads
or flushes. Body search adds one verified prefix read per candidate and
uses the existing 2,048-row and 16 MiB query caps. A named assignment
adds no file reads or flushes to a normal change. It plans from the
current in-batch row and uses the same verified file read and two existing
ledger phases as a built-in change. Midnight refresh reuses normal live
queries and performs no persistent writes. Public Rust APIs have
in-memory cost and behavior documented. Host tests cover real bundle
reload and the preview's idempotent backfill.

## Simplification opportunities

- Reuse the format's operation plan for more built-ins as the host
  supplies saved-item, record and viewer-state context. That would
  reduce validation split between the engine and the host.
- Retain typed fields in the per-space query index to avoid JSON-to-format
  conversion for every query. Measure first with bounded searches that
  include Markdown bodies.
- Give the format engine a compiled empty collection constructor so
  ad hoc reads can drop the host's synthetic Markdown definition.
