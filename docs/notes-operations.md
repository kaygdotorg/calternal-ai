# Notes operations

The official `calternal-notes` plugin declares the `notes` kind. Its native
engine stores note bodies in Markdown and serves the same operations to the
CLI, clients, and agents. See the [glossary](glossary.md) and
[address rules](addresses.md).

## Find notes and tags

| Operation | Input | Result |
| --- | --- | --- |
| `notes.list` | Space UUID, optional `tag` name | Up to 100 active notes, newest first |
| `notes.lookup` | Space UUID, literal `prefix` | Up to 100 active notes in folded title order, with the exact relative `link_target` to insert in a Markdown link |
| `tags.list` | Space UUID | Up to 100 names and counts in name order |
| `tags.lookup` | Space UUID, literal `prefix` | Up to 100 matching names and counts in name order |

A picker can send an empty prefix to start at the first name. Title matches
fold Unicode case. Tag names are case-sensitive, as in the reference Notes
engine. Body tags such as `#work/plans` and the `tags:` frontmatter property
use the same grammar as the reference app. Frontmatter accepts comma-separated
values, bracketed lists, and block lists. Fenced and inline code, URL
fragments, and link destinations do not count as body tags. Each note counts
once for a name and each of its parents. Filtering by `work` includes a note
tagged `work/plans`.

Each list and lookup returns one page. Send the returned `next_cursor` as
`cursor` with the same account, space, operation and filters to continue.
When the last page omits `next_cursor`, the list is complete. Restart at the
first page if the server has restarted or the cursor is rejected.

Pages seek from the last delivered sort key in the current index. The note
UUID breaks ties in update time or folded title. Each page reflects committed
changes at the time it is read. A note whose title or timestamp changes can
move across the page boundary. Refresh from the first page to see newly
inserted or moved rows before that boundary.

A warmed space serves each page with one indexed SQLite statement and no
note-file reads or flushes. Tagged notes retain update order and probe the
term index by exact label and note ID for each candidate from the live-note
index. Tag counts cover the live-note index entries for each returned name.
The backend projects tags and relative link targets from saved Markdown
revisions.

## Title and links

The first H1 supplies a note's display title and the basis for its filename.
A headingless note uses the title supplied on creation or its current title
on a save. An explicit title on update also replaces an existing H1.
`notes.rename` takes the note's UUID, new title, and `base_revision` from
the list. It sets or inserts the H1 and uses the same filename allocator as
save. Collisions take the next available numeric suffix.

Relative inline Markdown links such as `[Plan](<Plan.md#^block-id>)` remain
readable from the space's files. Rename plans from the active Notes backlink
index in the same space. It preserves anchors and authored labels and updates
labels that match the old title or filename stem. Stable `/notes/<uuid>`
addresses keep their identity.

Rename and title-changing saves return the revised note and a `job_id`. The job places the root at
its selected destination before placing dependent edits. A late competing
destination cancels the unpublished plan and preserves the original note.
At most eight
source edits run in a turn. A larger job resumes its stored source revisions
after a restart. Sources edited after planning are skipped to protect
their contents. `notes.rename.status` reports remaining sources and skips.
`notes.rename.undo` claims the original root path and reapplies the previous
revisions as one inverse job. Undo requires the affected notes to still have
the revisions the original job wrote. A later concurrent edit can be skipped
during the inverse, with progress reported by the same status operation.

## Trash

| Operation | Target | Effect |
| --- | --- | --- |
| `notes.delete` | Note UUID | Moves the file to its space's `.trash/<uuid>.md` and preserves history |
| `notes.restore` | Note UUID | Restores the same UUID and body at the former filename when available, using normal collision allocation otherwise |
| `trash.list` | Space UUID | Up to 100 trashed notes, newest first, with `updated_at` and `days_remaining` |
| `trash.purge` | Note UUID | Permanently removes a trashed note's file and content history |
| `trash.empty.preview` | Space UUID | Returns the count, retained version bytes, and confirmation for the current selection |
| `trash.empty` | Space UUID and confirmation | Starts one irreversible, resumable job for that exact selection |
| `trash.empty.status` | Space UUID and job UUID | Returns the selected count and bytes, purged count, skipped count, and state |

The address resolver returns `active`, `in_trash`, or `deleted` for an
authorized caller. Ordinary note reads, writes, and pickers see only active
notes. A foreign caller receives the same not-found response as for an
unknown UUID. A purged note retains its UUID tombstone so its address can
report the deletion. Editors have `content.write` for delete and restore.
Permanent removal and its preview require `space.settings`, held by the Manager
and Owner roles. A restored note gets its original path if free, or a numbered
path when occupied. Placement never replaces a competing destination.

Trash retention defaults to **30 days**. An operator can set
`CALTERNAL_NOTES_TRASH_RETENTION_DAYS` to a positive whole number of days.
The worker checks at startup and hourly, with short bounded turns while
work remains. Days remaining rounds up, with zero indicating eligibility.
A restore followed by another delete starts the retention period again.

Empty Trash confirms a snapshot of the deleted note revisions. If it changes
before admission, preview again. The worker checks each confirmed revision
before purging, so a later restore or delete stays outside the original
selection. Each turn processes at most 16 candidates. Purge uses the normal
guarded unlink and deletes the retained revisions, updating logical file usage
through the existing filesystem hook. SQLite can retain freed pages for reuse
and backups follow their own retention. A changed on-disk file is preserved.
Older body receipts cannot deliver purged contents.

The Notes jobs are stored in the space's content database. Newly started
jobs prune older finished jobs under the existing request receipt retention
policy. Jobs with work remaining are retained for recovery.

## CLI examples

The CLI returns one page of up to 100 rows. Repeat the same command with
`--cursor` and the continuation shown in the terminal or in `--json` output.

```sh
calternal notes list --space SPACE_UUID --tag work
calternal notes lookup --space SPACE_UUID Plan
calternal tags lookup --space SPACE_UUID work/
calternal notes rename NOTE_UUID --base-revision REVISION --title 'New Plan'
calternal notes rename-status --space SPACE_UUID JOB_UUID
calternal notes undo-rename --space SPACE_UUID JOB_UUID
calternal notes delete NOTE_UUID
calternal trash list --space SPACE_UUID
calternal notes restore NOTE_UUID
calternal trash purge NOTE_UUID --json
calternal trash empty --space SPACE_UUID
calternal trash empty --space SPACE_UUID --confirm CONFIRMATION
calternal trash status --space SPACE_UUID JOB_UUID
```
