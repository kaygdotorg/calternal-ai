# Notes jobs and moving existing files

[Forgejo issue #47](https://git.kayg.org/kayg/calternal-ai/issues/47).
The Trash and rename decisions are recorded in
[Notes operations](../notes-operations.md).

## Notes jobs

- Trash lists the rounded-up days remaining for each note. Automatic expiry
  defaults to 30 days and takes at most 16 candidates per space per turn.
- Empty Trash previews the count and retained revision bytes, then admits a
  confirmed set of exact revisions. The per-space job persists each outcome
  with purge finalization. It releases the space writer between items.
  A restored or subsequently deleted revision stays outside that selection.
- Editors can move notes to and from Trash. The Manager and Owner receive
  `content.purge` for single permanent purges, Empty Trash and its preview.
- Rename uses the inherited exclusive placement primitive before editing
  inbound links. A late competing destination cancels the unpublished plan
  so a fresh rename can replan. The durable job continues in turns of at most eight
  sources. Undo claims the original root path, then reapplies the recorded
  prior source revisions. Conflicting later revisions are preserved.
  Restore uses numbered filenames when its previous name is occupied.
- Auth version 15 follows the production baseline and content versions
  7–8 follow live rooms through the shared runner. Auth version 15 is incompatible
  because the previous writer still allows Editors to purge. Each new
  transition has an independent test. Conformance recovers an interrupted
  Empty Trash admission with existing content.

Purge unlinks owned Markdown files through the filesystem usage hook and
removes stored content versions. Calternal-AI's owning-space quota
enforcement is still pending, so there is no live quota balance to credit.
The 100,000-item workspace regression covers confirmation and frozen
selection. The HTTP checks cover execution, retry, restart and permissions
on small disposable spaces.

## Moving existing Notes files

The source check covered the currently installed `c98705d7` and the prior
`9f800564` baseline. The current lane keeps the same file-discovery behavior.
[Current production source](https://git.kayg.org/kayg/calternal-ai/src/commit/c98705d74bcf).

A saved Note belongs in
`<userdata-ssd>/spaces/<SPACE_UUID>/Notes/<Title>.md`. The checked-in cloud
deployment places that directory under `/srv/calternal-ai/userdata-ssd` on
the host and `/data/userdata-ssd` inside the container.
[Deployment configuration](../../deploy/cloud.conf) ·
[Container root mounts](../../deploy/calternal-ai.container).

Files that simply appear there are **left untouched and ignored as Notes**.
This holds when the server is running, after a restart, and during a search
rebuild. A `calternal-id` in a loose file grants no route. The file receives
no content head, history, journal entry, search hit, or import error. Filename
allocation sees the existing directory entry and preserves it by choosing a
numbered name on collision.
[Filename and placement code](../../crates/content/src/files.rs) ·
[First-use recovery](../../crates/content/src/recovery.rs) ·
[Search rebuild source](../../crates/content/src/search/source.rs).

Replacing or moving an already indexed file is a different case. A search
rebuild reads it only at the recorded path with the saved hash. On a mismatch
or missing path, it uses the committed revision. A later app write can detect
the outside change, block that item, and preserve the file.
[Search rebuild source](../../crates/content/src/search/source.rs) ·
[Guarded write code](../../crates/content/src/files.rs).

To create visible notes today, keep the original files in a separate staging
folder and use the authenticated command once per note while the server runs.

```sh
cl notes create --space SPACE_UUID --title 'Title' --markdown-file /staging/Title.md
```

This creates a new UUID and history. The host writes its assigned UUID into
the placed file. To avoid carrying the old ID as stored Markdown text, remove
the old `calternal-id` line from the staging copy before submitting. If the
Markdown has an H1, it supplies the title used for allocation. There is no
restart step, and ID-preserving adoption is currently unavailable.
[CLI file input](../../crates/cli/src/main.rs) ·
[Notes creation](../../crates/server/src/store.rs).

## Smallest adoption operation

Add an explicit authenticated `notes.adopt` operation for one existing space,
using `content.write`. Preview a bounded page of regular Markdown files in
the top-level Notes folder using guarded, non-symlink opens. Report invalid
metadata, duplicate UUIDs and occupied paths before confirmation. On apply,
recheck the exact file bytes and path, preserve valid unclaimed UUIDs, and use
the ordinary content transaction and outbox to record the first revision,
route, labels, links and journal event. Space search then follows its existing
journal path. An exact path, UUID and byte match should be a no-op on retry.
Files with conflicting UUIDs or paths stay in place for resolution. This keeps
adoption explicit and leaves the existing files untouched.

## Simplification opportunities

Keep file adoption on the existing content and outbox path. That gives imports
the same idempotency, search publication and conflict guards as ordinary Notes
without introducing a second watcher or file index.
