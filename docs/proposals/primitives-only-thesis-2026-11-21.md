# A primitives-only host · thesis · 2026-11-21

Status: the coordinator's starting position for one night of design work. It is not
decided. Each section lane tests it, corrects it and turns its part into a specification;
the integrated design and the questions go to the owner in
`docs/morning-2026-11-21.md`.

## Why

Revision 14 of the plugin API grew to 2,687 lines, about 60 concepts for a Chat-class
plugin and 312 numeric limits. The host learned chat vocabulary (entries, read marks,
quotes, consent modes, one successor per item). Four server slices were built as
libraries and were still far from one running Send. The owner's direction
(2026-11-20): the server is primitives only; plugins are powerful, versatile and
bounded; first-party plugins are not special; security stays in the host.

## What the owner has decided (binding)

- The server is primitives only. Disabling an official plugin that provides important
  functionality warns the person.
- One plugin may call another plugin's operations.
- Userdata is strictly files. Appdata is rebuildable and uses whatever is fastest.
- Drawing: a closed component catalogue plus a sealed frame. No plugin code in clients.
- Agents reading other people's writing is enforced by the host.
- A sidebar section is a collection shown small. A collection is a saved query over
  items of any kind plus hand-added items, with big and small views. Each kind says
  how it looks as a row, a card and a cell. A sidebar is a collection of collections,
  stored as a visible file. The middle is a canvas of windows.
- An app is as dumb as possible: kinds, operations and a default arrangement of
  collections.
- Apps are made of several plugins with declared dependencies. Who picks a part is
  split by effect (what only you see: per person; what others receive: per space). A
  missing part falls back or shows an error that suggests the official part.
- Spaces only, by UUID. Roles are reader, editor, manager, owner. rusqlite only.
  Anything stored as a file is Markdown unless that does not fit.

## The thesis in one paragraph

The host is a file server that knows who is asking. It stores files, answers questions
about them, runs plugin code in a box, routes calls after one permission check, and
hands interface descriptions to clients. It knows nothing about chats, notes, mail,
agents or providers. Everything a person would call a feature is a plugin, built from
the same seven primitives that a stranger's plugin gets.

## The seven primitives

1. **Files.** Items are files in a space's userdata. Two ways to write: replace a file,
   or append a record to it. A record is a small block with a stable anchor. The host
   stamps every write with its origin (person, plugin, run) and keeps that stamp where a
   plugin cannot forge it. Rename, move, trash and restore are host operations. A kind
   is a schema over a file's frontmatter, declared by a plugin.
2. **Questions.** The host indexes frontmatter, text, links and origin stamps into
   appdata and answers queries over them with live windows. A collection is a saved
   query plus hand-added items, stored as a file. Lists, sections, boards, search and
   backlinks are all the same engine. The index can always be rebuilt from the files.
3. **People and permissions.** People, groups, spaces, roles and per-file sharing. A
   plugin holds a grant (which kinds and operations it may use) that the space owner
   approves at install. Every action runs with the intersection of the plugin's grant
   and the acting person's rights. One extra rule, for any shared file: a run acting
   for an agent may not read bytes whose author has not granted that plugin access.
4. **Running code.** Plugin code runs in a sandbox with no ambient authority. Three
   things start it: a call, a time, or a change (a file or a query result changed).
   Long work and agent programs run on a machine; their output is appended to an
   ordinary file, so a restart or a dropped link means "sync resumes". Stop ends the
   process. Work in progress is one table with a state column.
5. **Calls.** One table from name to handler holds host methods and every plugin's
   published operations. Plugins call the host and each other through it. Each call is
   checked once, at that table, and written to an audit trail.
6. **Drawing.** A plugin describes interface with the catalogue's components; clients
   draw it natively. The shell itself (app card, sidebar, canvas windows, right panel)
   is made of replaceable parts filled by descriptions. Collections supply every list.
   A sealed frame covers what the catalogue cannot express.
7. **The outside.** Network requests to declared destinations, secrets that plugin code
   can use but not read, and notifications to the person's devices, all through the
   host.

## What this deletes from revision 14 (to be confirmed by the sections)

Attempts, fences and successors as host concepts (a run is a process plus a file).
The output spool, relay window, acknowledgements and reattachment proof. Attested
prefix and suffix framing (the origin stamp per record replaces it). Spans and checked
quotes as copied bytes (a quote is a reference to a record's anchor). Entry views and
their vocabulary (a chat is a file of records drawn by a list component). The
viewer-private record (per-person state is a small file). Contribution kinds and the
exchange protocol (a part is a plugin that fills a named slot; a harness is a plugin
that runs a program). Alarm grace and missed bits. The chat file format in the host
specification. Layout limits. Migration chains. Two validators.

## What must not get weaker

- A plugin cannot read or write outside its grant, whatever another plugin asks it to do.
- A plugin calling another plugin never gains rights: the callee runs with the
  intersection of the caller's grant, its own grant and the person's rights.
- Agents see other people's writing only with those people's grant.
- A hostile or broken plugin cannot stall the space, fill the disk, or outlive Stop:
  quotas for storage, compute, calls and wake-ups are host-enforced.
- Clients never run plugin code outside the sealed frame.
- Everything stored has a bound or a retention.
- The request path has no installation-wide lock and no blocking call on an async
  thread; the index is the only thing that makes a query fast.

## Questions the sections must answer

- Are seven primitives enough to build Chat (with people, agents, consent, quotes,
  delete for everyone, running through a restart), Notes (collections, history, links),
  Mail, and a ten-line list app? Where does each need something the list lacks?
- Which of the seven can still be removed or merged?
- What does a plain app author have to learn? The target is about fifteen concepts.
- Record files: how does a Markdown file stay readable and hand-editable while the host
  treats it as records with stamps? Where do the stamps live?
- How fast is each primitive on the request path, in reads and flushes?
- For each primitive: how is a hostile plugin contained?
- What survives from the code written so far, and what is thrown away?
