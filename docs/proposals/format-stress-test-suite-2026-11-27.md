# Stress test: the rest of the suite in the format · 2026-11-27

Status: **paper prototype**, no code. This repeats
[the Money, Mail and Chat stress test](format-stress-test-money-mail-chat-2026-11-24.md)
for the other seven apps: Calendar, Tasks, Workouts, Photos, Files,
Documents and Notes. Each is written out in the format as it stands after
rounds 29 to 32 (morning report): kind files, collections, a sidebar,
`plugin.md`, and every coded operation with the calls it uses. Two things
that are not apps follow: the iOS keyboard with the native Apple surfaces,
and serving CalDAV, CardDAV and WebDAV to outside clients.

Behaviour comes from the old app's decisions, read with git from
`/home/kayg/Developer/calternal` at `origin/dev`: ADRs 0023, 0025, 0026,
0055, 0063, 0064, 0066, 0067 and 0068, the recorded Files and Photos
decisions (`ops/orchestrator/memory/calternal-decisions-{files,photos}.md`),
and [docs/inheritance-audit.md](../inheritance-audit.md). The old app has no
Workouts or Documents decisions. For those two, behaviour is taken only from
the owner's reference apps (round 21's suite list: RP Hypertrophy and
MyoAdapt; Paperless-ngx), and anything product-shaped is a question.

**What the format has today**, used as the baseline:

- **Kind keys:** `kind`, `name`, `folder`, `editable`, `format`, `fields`,
  `records`, `row`, `card`, `cell`, `page`, `repeat`, `operations` (`add:
  fields`, `change`, `remove`, `name: set f = v[, when c][, swipe|menu|primary]`,
  `name: code`), `checks`, `together`, `search`, `template`, and `item:
  folder with <file>`.
- **Collection keys:** `collection`, `title`, `from`, `where`, `sort`,
  `group`, `total`, `fields`, `views`. The views are `list`, `table`,
  `board`, `gallery`, `calendar`, `timeline`, `chart`, `compact`, `strip`
  and `map`.
- **Sidebar files:** a list of collections.
- **`plugin.md` keys:** `plugin`, `name`, `version`, `icon`, `needs`,
  `sends out`, `calls`, `secrets`, `programs`.
- **Fifteen calls:** `s.query`, `s.read`, `s.write`, `s.fetch`,
  `s.connect`, `s.notify`, `s.job`, `s.run`, `s.call`, `s.state`,
  `s.cache`, `s.me`/`s.space`/`s.event` (counted as one), and the three
  added in round 31: `s.share`, `s.send` (input to a run) and `s.stop`
  (stop a run).
- **Six events:** `on.call`, `on.time(field)`, `on.schedule(repeat)`,
  `on.change(kind or collection)`, `on.outside(name)` and
  `on.connection(name)`.

**Round 33 (2026-11-27)** answered the items this test was told to treat as
open, and the answers are used throughout:

- **G5:** `change: manager` in a kind file.
- **G13:** slots declared in `plugin.md`.
- **G14:** a plugin may add a collection to another app's sidebar,
  declared, shown at install and removable.
- **F1:** `per month` fields with `previous()`.
- **F2:** `totals:` with `all(kind)`.
- **F3:** one link hop.
- **F4:** the cap counts after the condition.
- **The engine:** one Rust engine, also as Wasm in the browser.

The coordinator also decided, the same day, that a plugin may add fields,
operations and collections to another plugin's kind, declared and shown at
install.

Every place where I had to invent something is marked `# invented: #N`. N is
the row number in the table in §10. Under AGENTS.md "Who decides what", each
row says whether it is a high-level question **for the owner** (security
limits, reaching other people, money, data loss, product direction) or a
detail **for the coordinator**. A detail row comes with the most versatile
answer I recommend.

## 1. Calendar (`calternal-calendar`)

Inherited: ADR 0067 and ADR 0068.

- A line in a daily note that starts with a time is an event. A start-only
  event shows as 15 minutes long.
- An event has exactly one structured owner: its event note
  (`Calendar/<title>.md`), or an `events` entry in its daily note until it
  is promoted.
- Daily, weekly, monthly and yearly notes live in `Calendar/`.
- Each top-level `#area/<tag>` is a calendar, and an event with no area
  goes to **Journal**. Settings names calendars and sets their colour and
  default alerts.
- Planned or logged is worked out from the time when the event is read,
  never stored. "Didn't happen" is explicit.
- A repeating event is one series note. Occurrences are shown but not
  written until acted on, and a moved occurrence becomes a per-day note
  with `series` and `occurrence`.
- Alerts are VALARMs delivered by CalDAV clients. Calternal does not
  deliver them itself.
- **Connected calendars** come from any CalDAV server. Their events become
  local event notes with a `source` map. Your notes on someone else's
  event never leave Calternal. Nothing is sent upstream unless you own the
  event, and `people` links never invite anyone.

Editable outside.

```markdown
<!-- plugin.md -->
---
plugin: calternal-calendar
name: Calendar
version: 1.0.0
icon: calendar
format: 1
needs: [calternal-notes >=1, calternal-contacts >=1 optional]    # invented: #44
sends out: a server you name, caldav.icloud.com, caldav.fastmail.com, apidata.googleusercontent.com
secrets: calendar sign-in, per accounts
calls: []
settings:                                 # invented: #11 (ADR 0055 typed settings)
  start of week: choice(Monday|Sunday), default Monday
  default event length: duration, default 1h
---
Your calendars come from area tags in your notes; connected calendars come
from any CalDAV server you sign in to. Never invites anyone.
```

```markdown
<!-- kinds/events.md -->
---
kind: events
name: Event
folder: Calendar
file: {title}                             # invented: #20
editable: outside
format: 1
lines: time in calternal-notes.days       # invented: #1 (a time line in a daily note is an event)
fields:
  title: text, required, heading          # invented: #22
  start: when, required                   # invented: #2 (a day for all-day, a time otherwise)
  end: when
  all day: yes/no, default no
  time zone: text
  location: text
  people: links(contacts)                 # invented: #44
  calendar: link(calendars)               # override; else from tags
  tags: tags
  status: choice(did not happen)
  repeat: repeat
  exceptions: dates                       # invented: #2 (skipped occurrences, EXDATE)
  series: link(events)                    # a per-day override of a series
  occurrence: when                        # the original slot, unchanged by moves
  alert: durations                        # invented: #2
  source: data                            # invented: #3 (account, calendar, uid, href, etag)
  remote description: long text           # the connected event's DESCRIPTION, kept apart
  own calendar: = calendar or first(under(tags, "area"))   # invented: #8, #9
  ends: = end or start + if(all day, 1d, 15m)
  logged: = ends <= now()                 # invented: #43 (re-evaluated at the event's end)
repeat: repeat
together:
  start, end, all day, time zone
checks:
  empty(end) or end >= start
row: start, title
page: start, end, location, people, calendar, alert
search: title 3, location 2, body
ical: VEVENT                              # invented: #41
  title: SUMMARY
  start: DTSTART
  end: DTEND
  location: LOCATION
  repeat: RRULE
  exceptions: EXDATE
  alert: VALARM
  body: DESCRIPTION, unless source
operations:
  add: title, start, end, calendar, phrases   # invented: #16
  change: only title, body, calendar, when empty(source) or source.owned   # row 5 of the earlier test
  remove
  did not happen: set status = did not happen, when not logged, menu
  skip: code                              # one occurrence of a series
  move occurrence: code
  move all future: code
---
An event: one per file, or a time line in a daily note until it gets its own
note. The body is your notes about it; for a connected event they never
leave Calternal.
```

```markdown
<!-- kinds/calendars.md -->
---
kind: calendars
name: Calendar
folder: Calendar/Calendars
editable: outside
fields:
  name: text, required
  tag: text                               # the area tag, e.g. area/work; empty for Journal and connected ones
  colour: colour                          # invented: #2
  default alerts: durations
  account: link(accounts)                 # set for a connected calendar
  read only: yes/no, default no
row: name
operations:
  add: name, tag, colour
---
```

```markdown
<!-- kinds/accounts.md -->
---
kind: accounts
name: Connected account
folder: Calendar/Accounts
editable: operations
fields:
  name: text, required
  server: url, required
  sync: repeat, default FREQ=MINUTELY;INTERVAL=15
  last synced: time
  status: choice(Working|Needs sign-in|Server unreachable)
row: name, status
operations:
  add: code                               # sign in, discover calendars
  remove: code                            # keeps the events' notes, drops `source`
  sync now: code, primary
---
```

```markdown
<!-- kinds/days.md : the periodic notes Calendar owns (0067) -->
---
kind: days
name: Day
folder: Calendar
file: {date:YYYY-MM-DD}                   # invented: #20
unique: date                              # invented: #21
editable: outside
fields:
  date: date, required
row: date
operations:
  add: date
  today: open where date = today(), primary   # invented: #21 (open, adding it if missing)
---
A daily note. Starts empty, no template, no heading (0026).
```

Week, month and year notes are the same kind with `period:
choice(day|week|month|year)` and file names `2026-W48`, `2026-11` and
`2026`. That is one more `file:` pattern per period (#20).

```markdown
<!-- collections: one block per file -->
---
collection: week
title: Week
from: events
views:
  calendar: start .. ends, colour own calendar.colour    # invented: #5 (expands repeats), #2 (colour)
  list: start, title
---
collection: today
title: Today
from: events
where: day(start) <= today() and day(ends) >= today()
sort: start
views:
  list: start, title
  compact: start, title
---
collection: by-calendar
title: Calendars
from: calendars
where: empty(account)
views:
  list: name
---
```

```markdown
<!-- sidebars/calendar.md -->
---
sidebar: calendar
title: Calendar
---
- [Today](<../collections/today.md>)
- [Week](<../collections/week.md>)
- each: calendars, where empty(account)       # invented: #10 (one entry per calendar)
- each: calendars, where not empty(account), under "Connected"
- [Accounts](<../collections/accounts.md>)
```

**Coded operations:**

| Operation | Signature | Calls |
| --- | --- | --- |
| `accounts.add` | `(server, sign-in)` | `s.fetch(PROPFIND current-user-principal, calendar-home, { secret })`; `s.write([add account, add a calendar per collection])` |
| sync | `on.schedule(sync)` on accounts | `s.fetch(REPORT sync-collection with the stored token)`; `s.write([add, change or remove events with source])`; the token and ETags saved durably (#4) |
| push upstream | `on.change(events)` where `source` and owned | `s.event.before` (#42) to see which fields changed; `s.fetch(PUT with If-Match)`; on 412, fetch and keep both versions; queue the change durably while offline (#4) |
| `skip` | `(event, occurrence)` | `s.write([change exceptions + occurrence])`; a log entry for a series with `log` (#18) |
| `move occurrence` | `(event, occurrence, new start)` | `s.write([add a per-day note with series and occurrence])` |
| `move all future` | `(event, from occurrence, new start)` | `s.write([change the old series' repeat to UNTIL, add a new series, move later per-day notes])` in one write |
| `accounts.remove` | `(account)` | `s.query(events where source.account = …)`; `s.write([change source = empty …, remove account and its calendars])` |

Serving CalDAV to Apple Calendar is §9, not this plugin.

## 2. Tasks (`calternal-tasks`)

Inherited: ADR 0066 and ADR 0068.

- A checkbox is a task only when it has a child link to its task note.
  Plain checkboxes stay in **Loose ends**, with Track.
- The task note's frontmatter is the only home of a task's fields, and
  every checkbox line that points at it mirrors its status.
- When (`scheduled`) and Deadline (`due`) are each a date or a time.
- `log` lists the outcomes. Time spent comes from linked logged events and
  never completes a task.
- Repeats run on a schedule or after you finish, with goals, skips and
  streaks, and no pile-up of missed ones.
- The sidebar lists are Today, Upcoming, Anytime and Someday, then views
  and tags.
- One group-by is shared by List, Board and Matrix. Dropping a task into a
  group sets the property behind the group.
- Manual order is kept per list and group, outside the files.
- Task notes live in `Tasks/` (precedence fix 1 of the audit, overriding
  `Notes/Tasks/`).
- Tracked tasks sync to Apple Reminders as VTODO. That is §9.

Editable outside.

```markdown
<!-- plugin.md -->
---
plugin: calternal-tasks
name: Tasks
version: 1.0.0
icon: checklist
format: 1
needs: [calternal-notes >=1, calternal-calendar >=1 optional]
reads: calternal-calendar.events          # invented: #31 (time spent from logged sessions)
sends out: nothing
calls: []
settings:                                 # invented: #11
  urgent within: duration, default 2d
  folder for new tasks: text, default Tasks
---
Tasks are notes with a status. A checkbox in any note can become one.
```

```markdown
<!-- kinds/tasks.md -->
---
kind: tasks
name: Task
folder: Tasks
file: {title}                             # invented: #20
editable: outside
format: 1
lines: checkbox in calternal-notes.notes, calternal-calendar.days   # invented: #1 (the box mirrors status)
fields:
  title: text, required, heading          # invented: #22
  status: choice(todo|doing|done|cancelled|someday), default todo
  scheduled: when                         # invented: #2 (When)
  due: when                               # Deadline
  priority: choice(urgent|high|medium|low)
  assignee: link(contacts)                # invented: #44; assigning reaches a person, so it asks first
  tags: tags
  pinned: yes/no, default no
  depends on: links(tasks)
  waiting on: text
  repeat: repeat
  repeat from: choice(schedule|completion), default schedule
  repeat after: link(calternal-calendar.events)
  repeat offset: duration
  repeat goal: number
  completed: time
  cancelled: time
  log: records(done: time, due: date, skipped: date)   # invented: #18
  reminders: durations
  source: data                            # invented: #3 (the Reminders binding)
  open: = status = "todo" or status = "doing"
  waiting: = count(depends on, open) > 0 or not empty(waiting on)   # invented: #8
  first date: = due or scheduled
  overdue: = open and first date < today()
  urgent: = open and days(today(), first date) <= days(settings.urgent within)   # invented: #11
  next: = next(repeat, after: if(repeat from = "completion", last(log.done), scheduled))   # invented: #6
  streak: = streak(log)                   # invented: #19
  time spent: = sum(from(calternal-calendar.events.task, logged).length)   # needs #31
repeat: repeat
together:
  repeat, repeat from, repeat after, repeat offset
  scheduled, due
row: title, first date, priority
page: status, scheduled, due, priority, tags, repeat, depends on
search: title 3, tags 2, body
template: none
operations:
  add: title, scheduled, due, priority, tags, line in today, phrases   # invented: #1, #16
  change
  remove: code                            # trash the note and take its lines out of every note
  done: set status = done, set completed = now(), when open, swipe, primary
  start: set status = doing, when status = "todo", menu
  won't do: set status = cancelled, set cancelled = now(), when open, menu
  someday: set status = someday, menu
  schedule: set scheduled = ask, menu     # invented: #7 (asks for the day)
  pin: set pinned = not pinned, menu
  skip: code, when not empty(repeat)
  untick: code, when status = "done"
  track: code                             # a plain checkbox becomes a task
  duplicate                               # invented: #12
  convert obsidian tasks: code
---
One task per note. The box on every line that links here follows `status`.
```

```markdown
<!-- collections: one block per file -->
---
collection: today
title: Today
from: tasks
where: open and status != "someday" and (scheduled <= today() or due <= today())
group: status
sort: manual, pinned desc, first date, priority   # invented: #13
drop: set scheduled = today()             # invented: #14
views:
  list: title, first date, priority
  board: status
  matrix: priority in (urgent, high), urgent   # invented: #15
---
collection: upcoming
title: Upcoming
from: tasks
where: open and status != "someday" and first date > today()
group: day(first date)
sort: manual, first date
expand: repeat                            # invented: #5 (each future occurrence)
drop: set scheduled = today() + 1d
views:
  list: title, first date
---
collection: anytime
title: Anytime
from: tasks
where: open and status != "someday" and empty(scheduled) and empty(due)
sort: manual, created                     # invented: #23
drop: set scheduled = empty, set due = empty
views:
  list: title, tags
---
collection: someday
title: Someday
from: tasks
where: status = "someday"
drop: set status = someday
views:
  list: title
---
collection: logbook
title: Logbook
from: tasks
where: status = "done" or status = "cancelled"
sort: completed desc
group: day(completed)
views:
  list: title, completed
---
collection: loose-ends
title: Loose ends
from: lines(checkbox) where empty(task)   # invented: #1 (plain checkboxes in any note)
views:
  list: text, note
---
collection: habits
title: Habits
from: tasks
where: not empty(repeat) and open
views:
  list: title, next, streak
---
```

```markdown
<!-- sidebars/tasks.md -->
---
sidebar: tasks
title: Tasks
---
- [Today](<../collections/today.md>)
- [Upcoming](<../collections/upcoming.md>)
- [Anytime](<../collections/anytime.md>)
- [Someday](<../collections/someday.md>)
- [Habits](<../collections/habits.md>)
- [Logbook](<../collections/logbook.md>)
- [Loose ends](<../collections/loose-ends.md>)
- each: under(tags, ""), from tasks       # invented: #9, #10 (one entry per top-level tag)
```

**Coded operations:**

| Operation | Signature | Calls |
| --- | --- | --- |
| `track` | `(line)` | `s.read(note)`; `s.write([add task note, change the line: add sub-bullet link, add from])` as one write |
| `remove` | `(task)` | `s.query(lines linking the task)` (#1); `s.write([remove note, change every note that holds a line])` |
| `skip` | `(task)` | `s.write([append to log skipped = next, change scheduled = the following date])` |
| `untick` | `(task)` | `s.write([remove the last log entry, restore scheduled])` |
| box reconciler | `on.change(calternal-notes.notes)` | `s.event.before` (#42) to compare boxes against their base; `s.write([change status of each task whose box changed])`, newest edit winning (0066 TQ10) |
| `convert obsidian tasks` | `(notes)` | `s.query`; `s.write(changes, { preview })` (#17) shows the dry run; then the same write for real |
| repeat after a tick | `on.change(tasks)` where `done` and `repeat` | `s.write([change status = todo, scheduled = next, append log done])` |

With #1 in the shared core, mirroring the boxes on ticks made in the app
(including offline) needs no code. The reconciler only handles boxes
changed over WebDAV or in source mode.

## 3. Workouts (`calternal-workouts`)

There are no inherited decisions. The shape follows the owner's reference
apps (RP Hypertrophy, MyoAdapt):

- exercises;
- a training block (mesocycle) of some weeks, with a weekly plan;
- sessions logged set by set (weight, reps, reps in reserve);
- after each session, feedback (pump, soreness, joint pain, workload) that
  sets next week's sets and loads.

How much of that the app does in its first version is question O3.

Editable outside, so a person or an agent can read and fix a log in the
WebDAV mount.

```markdown
<!-- plugin.md -->
---
plugin: calternal-workouts
name: Workouts
version: 1.0.0
icon: dumbbell
format: 1
sends out: nothing
calls: []
settings:                                 # invented: #11
  unit: choice(kg|lb), default kg
  rest: duration, default 2m
---
Plans, sessions and sets. Logs are plain notes you can read anywhere.
```

```markdown
<!-- kinds/exercises.md -->
---
kind: exercises
name: Exercise
folder: Workouts/Exercises
editable: outside
fields:
  name: text, required
  muscle: choice(Chest|Back|Shoulders|Biceps|Triceps|Quads|Hamstrings|Glutes|Calves|Abs)
  equipment: choice(Barbell|Dumbbell|Machine|Cable|Bodyweight)
  best e1rm: = max(from(sessions.records.exercise).e1rm)   # invented: #34 (records across items)
row: name, muscle, best e1rm
operations:
  add: name, muscle, equipment
---
```

```markdown
<!-- kinds/blocks.md : a mesocycle -->
---
kind: blocks
name: Training block
folder: Workouts/Blocks
editable: outside
fields:
  name: text, required
  starts: date, required
  weeks: number, default 5
  deload: yes/no, default yes             # the last week is lighter
  week: = days(starts, today()) / 7 + 1
  ends: = starts + weeks * 7d
checks:
  weeks >= 1 and weeks <= 12
row: name, week
operations:
  add: name, starts, weeks
---
```

```markdown
<!-- kinds/plan-days.md : "Push A on Mondays" -->
---
kind: plan-days
name: Plan day
folder: Workouts/Plan
editable: outside
repeat: repeat
fields:
  name: text, required
  block: link(blocks), required
  repeat: repeat
  next: = next(repeat)
records:
  exercise: link(exercises), required
  sets: number, default 3
  reps: text, default 8-12
row: name, next
operations:
  add: name, block, repeat
  append record                           # needs the records answer (#34)
  change record
  remove record
---
```

```markdown
<!-- kinds/sessions.md : one per workout -->
---
kind: sessions
name: Session
folder: Workouts/Sessions
file: {date:YYYY-MM-DD} {plan day.name}   # invented: #20
editable: outside
fields:
  date: date, default today
  plan day: link(plan-days)
  done: yes/no, default no
  pump: choice(Low|Moderate|Great)
  soreness: choice(None|Healed early|Healed on time|Still sore)
  joint pain: choice(None|Some|A lot)
  workload: choice(Easy|Pretty good|Pushed my limits|Too much)
  volume: = sum(records.volume)
records:
  exercise: link(exercises), required
  weight: number kg                       # invented: #2 (unit from settings)
  reps: number
  reps in reserve: number
  planned: yes/no, default no             # written by progression; becomes no when logged
  volume: = weight * reps
  e1rm: = weight * (1 + reps / 30)
row: date, plan day, volume
page: records(exercise, weight, reps, reps in reserve)
operations:
  add: plan day, date
  today: open where date = today() and plan day.next = today(), primary   # invented: #21
  append record: exercise, weight, reps, reps in reserve, primary   # logging a set: must work offline (#34)
  change record: weight, reps, reps in reserve
  remove record
  rest: timer settings.rest, after append record   # invented: #35
  finish: set done = yes, when not done, primary
---
One session per file. Each set is a record line in the body, so a whole
workout is one readable note.
```

```markdown
<!-- collections: one block per file -->
---
collection: today
title: Today
from: sessions
where: date = today()
views:
  list: plan day, volume
---
collection: weekly-volume
title: Weekly volume
from: sessions.records                    # invented: #34 (a collection over records)
where: not planned
group: exercise.muscle
total: sum(volume), count
views:
  chart line: sum(volume) by week(date)
  table: exercise, weight, reps
---
collection: history
title: History
from: sessions
where: done
sort: date desc
views:
  list: date, plan day, volume
---
```

```markdown
<!-- sidebars/workouts.md -->
---
sidebar: workouts
title: Workouts
---
- [Today](<../collections/today.md>)
- [History](<../collections/history.md>)
- [Weekly volume](<../collections/weekly-volume.md>)
- [Exercises](<../collections/exercises.md>)
- [Blocks](<../collections/blocks.md>)
```

**Coded operations:**

| Operation | Signature | Calls |
| --- | --- | --- |
| progression | `on.change(sessions)` where `done` | `s.query(this exercise's last sessions and their feedback)`; `s.write([add next week's session with planned records: weight and set count from the feedback rule])` |
| new block | `(from block)` | `s.read(block)`; `s.write([add block, add plan days copied with new repeat starts])` |
| export to Apple Health | — | not a plugin call: a device bridge (#39) |

Weekly volume per muscle over years stays within the 1,000 cap only if the
cap counts records after the condition (F4 (a)). Per-week totals would be
per-week fields if F1 (b) is chosen. Today the chart's grouping does the
same job.

## 4. Photos (`calternal-photos`)

Inherited (Photos PH, PR and FP rounds, Files FS1 to FS3):

- **Originals and placement.** Originals are lossless and never written
  to. A photo is placed under `Photos/YYYY/YYYY-MM-DD` by its local capture
  day, and folders a person marks "Show in Photos" are indexed where they
  are.
- **Calternal's metadata** is kept in `.calternal/`, keyed by ID. An XMP
  file that came with the photo is never touched, and the app shows the
  union of both. Favourite is `xmp:Rating="5"`; Hidden is
  `calternal:hidden`.
- **Pairs.** Live Photos and RAW+JPEG pairs are one item, deleted and
  restored together. Exact duplicates are removed automatically;
  near-duplicates are only suggested.
- **Organising.** OCR always runs on the server. Faces are on by default
  and run on the server, and People and Places are separate. Search covers
  dates, places, people, cameras and OCR, plus CLIP-style semantic search.
- **Albums and sharing.** Albums are Collections. There is partner sharing
  (everything, or from a date; albums only or everywhere) and shared
  albums. "On this day" memories appear as cards in Home.
- **No editing** in version 1. Favourite and hide are organising, not
  editing.
- **Sidebar** sections: Library (timeline, Favourites, Hidden), Collections,
  People, Places.

```markdown
<!-- plugin.md -->
---
plugin: calternal-photos
name: Photos
version: 1.0.0
icon: photo
format: 1
sends out: nothing
programs: libvips, libheif, ffmpeg, ocr (PP-OCR), faces (YuNet, AuraFace), clip
calls: []
search: search photos                     # invented: #29 (semantic and face search)
slots:                                    # G13 (decided): face and OCR models are pluggable (LQ2)
  face model: one, recommended faces
settings:                                 # invented: #11
  faces: yes/no, default yes
  transcode videos: yes/no, default no
---
Your photos stay exactly as they were taken. Recognition runs on your server,
never elsewhere.
```

```markdown
<!-- kinds/photos.md -->
---
kind: photos
name: Photo
folder: Photos/{taken:YYYY}/{taken:YYYY-MM-DD}, plus marked folders   # invented: #20, #26
item: file(image/*, video/*), paired      # invented: #24, #25 (the original is the item)
editable: outside                         # uploads over WebDAV (PhotoSync); .calternal/ is refused
fields:
  taken: time, from metadata              # read from EXIF, never written
  camera: text, from metadata
  lens: text, from metadata
  place: place, from metadata
  duration: duration, from metadata
  favourite: yes/no, default no, in sidecar xmp:Rating=5   # #24 (PH-B)
  hidden: yes/no, default no, in sidecar calternal:hidden
  people: links(people), in sidecar       # written by the faces job
  place name: text, in sidecar            # offline reverse geocoding
  text: long text, in sidecar             # OCR
default where: not hidden                 # invented: #28
row: taken, place name
card: image
page: image, taken, camera, lens, place, people
search: place name 2, people 2, camera, text
operations:
  favourite: set favourite = not favourite, swipe, primary
  hide: set hidden = yes, menu
  unhide: set hidden = no, when hidden, menu
  remove                                  # the pair goes to the one Trash together
---
A photo or video, kept exactly as it arrived. What Calternal adds lives in
.calternal/, never in the file.
```

```markdown
<!-- kinds/people.md : a person recognised in photos (not a space member) -->
---
kind: people
name: Person
folder: Photos/People
editable: outside
fields:
  name: text
  contact: link(contacts)                 # #44
  photos: = count(from(photos.people))
  cover: link(photos)
row: name, photos
operations:
  merge: code, menu
  not this person: code
---
```

```markdown
<!-- collections: one block per file -->
---
collection: library
title: Library
from: photos
sort: taken desc
group: month(taken)
views:
  gallery: image
  strip: image                            # the viewer's bottom carousel (PH16)
---
collection: favourites
title: Favourites
from: photos
where: favourite
views:
  gallery: image
---
collection: hidden
title: Hidden
from: photos, including hidden
where: hidden
views:
  gallery: image
---
collection: places
title: Places
from: photos
where: not empty(place)
views:
  map: place
  gallery: image
---
collection: on-this-day
title: On this day
from: photos
where: month(taken) = month(today()) and day of month(taken) = day of month(today()) and year(taken) < year(today())
views:
  gallery: image
show in: home, as card                    # invented: #30 (G14 covers sidebars, not Home)
---
```

```markdown
<!-- sidebars/photos.md -->
---
sidebar: photos
title: Photos
---
- [Library](<../collections/library.md>)
- [Favourites](<../collections/favourites.md>)
- [Hidden](<../collections/hidden.md>)
- section: Collections                    # albums: any collection over photos
- each: people, sort photos desc, under "People"   # invented: #10
- [Places](<../collections/places.md>)
```

**Coded operations:**

| Operation | Signature | Calls |
| --- | --- | --- |
| describe new photos | `on.change(photos)` for new items | `s.job("describe", items)`: thumbnails are the host's (Files P1); the job reads metadata, OCR, faces and the CLIP vector; `s.write([change sidecar fields])`; vectors in `s.cache` |
| place a stray upload | `on.change(photos)` where the folder does not match `taken` | `s.write([move to Photos/YYYY/YYYY-MM-DD])` (#32) |
| `search photos` | `on.call(query)` | `s.cache` (vectors), `s.query`; returns ranked items |
| `people.merge` | `(person, into)` | `s.query(photos where people has person)`; `s.write([change people …, remove person])` |
| transcode | `on.change(photos)` where video and the setting is on | `s.job("transcode")`; the result is a large derived file in the cache (#33) |
| near-duplicates | weekly | `s.job("similar")`; `s.write` nothing; offers a review collection |

Partner sharing "everywhere" is question O1.

## 5. Files (`calternal-files`)

Inherited (Files F, FB and FD rounds, FS1 to FS6):

- **The tree.** One real folder tree that every other app gets its data
  from. The root is curated (Documents, Photos, Notes, Shared), internals
  are hidden, and "Show hidden" reveals them. Outside changes are picked up
  live.
- **Moves and links.** IDs survive moves and renames, and copies get a new
  ID. Every link to a moved path is rewritten.
- **Host features.** Previews are made on the server. Versions are kept
  for 30 days, then daily for six months, then monthly, and the one Trash
  keeps 30 days. Uploads are resumable. Dedupe is automatic, search is
  FTS5 per user, and quota is one per person.
- **Sharing.** Grants on a folder are inherited by what is in it, and
  nothing widens implicitly. Sharing has links (with optional expiry and no
  password, as ported on 2026-11-18) and shared folders.
- **Offline.** "Keep Downloaded", and an "Available Offline" section.
- **Layout.** A sidebar, a middle pane with drill-in and breadcrumbs, and
  an optional inspector shown only when something is selected. A list by
  default, with a grid switch remembered per folder. Columns can be added
  and removed.

Almost all of this is the host's Files part, not the plugin's. Files is
very nearly a description-only plugin.

```markdown
<!-- plugin.md -->
---
plugin: calternal-files
name: Files
version: 1.0.0
icon: folder
format: 1
sends out: nothing
calls: []
---
Every file in this space, in its real folders.
```

```markdown
<!-- collections: one block per file -->
---
collection: all-files
title: Files
from: files                               # invented: #27 (every file and folder, any kind)
where: not hidden or settings.show hidden
views:
  folders: name, edited, size, type       # invented: #27 (drill-in with breadcrumbs)
  gallery: preview, name
---
collection: recent
title: Recent
from: files
where: edited > today() - 30d
sort: edited desc
views:
  table: name, folder, edited
---
collection: large
title: Large
from: files
where: size > 100 MB
sort: size desc
views:
  table: name, folder, size
---
collection: offline
title: Available Offline
from: files
where: kept downloaded                    # a device fact the client supplies; never stored on the server
views:
  table: name, folder, size
---
collection: shared
title: Shared with me
from: files
where: shared with me
views:
  folders: name, shared by, edited
---
```

```markdown
<!-- sidebars/files.md -->
---
sidebar: files
title: Files
---
- [Files](<../collections/all-files.md>)
- [Recent](<../collections/recent.md>)
- [Shared with me](<../collections/shared.md>)
- [Available Offline](<../collections/offline.md>)
- [Large](<../collections/large.md>)
- folders: Documents, Photos, Notes       # invented: #27 (folder shortcuts)
```

**Coded operations:** none. Upload, New folder, move, rename, Copy link,
Share and Keep Downloaded act on any item and belong to the host. New note
is Notes' `add`. The previews, indexers and protocol adapters the audit
lists as extension points are host parts or other plugins.

## 6. Documents (`calternal-documents`)

Inherited:

- Documents is a separate app, "more like paperless-ngx" (FB6).
- Non-image attachments are placed in `Documents/YYYY/YYYY-MM-DD` (the
  audit's attachments row).
- PH26 left open whether Documents is arranged by date or by the person.
- FS1 to FS3: Calternal's metadata goes in `.calternal/`, and the person's
  files are never touched. The format's example 2.12, which put a Markdown
  file beside each scan, breaks FS1 and is replaced here.

From Paperless-ngx, not inherited:

- an inbox of new documents;
- correspondents, document types and tags assigned by matching rules (any
  word, all words, exact, regular expression, fuzzy, or learned);
- dates found in the text;
- an archive serial number;
- files renamed by a pattern.

```markdown
<!-- plugin.md -->
---
plugin: calternal-documents
name: Documents
version: 1.0.0
icon: doc
format: 1
needs: [calternal-mail >=1 optional]
reads: calternal-mail.messages            # invented: #31 (attachments of mail to file)
adds to:                                  # coordinator 2026-11-27: operations on another plugin's kind
  calternal-mail.messages: file to documents, menu, when has attachments
sends out: nothing
programs: ocr (PP-OCR), pdf (pdfium), classifier
search: search documents                  # invented: #29
calls: []
---
Scans and PDFs, read and sorted for you. The originals are never changed.
```

```markdown
<!-- kinds/documents.md -->
---
kind: documents
name: Document
folder: Documents/{added:YYYY}/{added:YYYY-MM-DD}   # invented: #20; C5 recommends by date
item: file(application/pdf, image/*, text/*)        # invented: #24
editable: outside
fields:
  title: text, in sidecar
  correspondent: link(correspondents), in sidecar
  type: link(document-types), in sidecar
  tags: tags, in sidecar
  date: date, in sidecar                  # found in the text, changeable
  serial: number, in sidecar              # archive serial number
  inbox: yes/no, default yes, in sidecar
  text: long text, in sidecar             # OCR
  added: time, from metadata
  pages: number, from metadata
checks:
  empty(serial) or count(from(documents.serial = serial)) = 1   # one item per number
row: title, correspondent, date
card: preview, title
page: preview, title, correspondent, type, date, tags, serial
search: title 3, correspondent 2, tags 2, text
operations:
  change: only title, correspondent, type, tags, date, serial, inbox
  done: set inbox = no, when inbox, swipe, primary
  remove
  rename by pattern: code, menu
---
```

```markdown
<!-- kinds/correspondents.md (document-types is the same shape) -->
---
kind: correspondents
name: Correspondent
folder: Documents/Correspondents
editable: outside
fields:
  name: text, required
  match: text
  matching: choice(Any word|All words|Exact|Regular expression|Fuzzy|Learned), default Learned
  documents: = count(from(documents.correspondent))
  last: = max(from(documents.correspondent).date)
row: name, documents, last
operations:
  add: name, match, matching
---
```

```markdown
<!-- collections: one block per file -->
---
collection: inbox
title: Inbox
from: documents
where: inbox
sort: added desc
views:
  list: title, correspondent, added
---
collection: by-correspondent
title: By correspondent
from: documents
group: correspondent
total: count
views:
  table: title, date
---
collection: all-documents
title: All documents
from: documents
sort: date desc
views:
  table: title, correspondent, type, date, serial
  gallery: preview, title
---
```

```markdown
<!-- sidebars/documents.md -->
---
sidebar: documents
title: Documents
---
- [Inbox](<../collections/inbox.md>)
- [All documents](<../collections/all-documents.md>)
- [By correspondent](<../collections/by-correspondent.md>)
- each: document-types, under "Types"    # invented: #10
```

**Coded operations:**

| Operation | Signature | Calls |
| --- | --- | --- |
| consume | `on.change(documents)` for new items | `s.job("ocr", items)`; then `s.query(correspondents, document-types)`, the matching rules and the classifier; `s.write([change sidecar fields])` |
| `file to documents` | an operation Documents **adds to Mail's messages kind** (coordinator, 2026-11-27), shown on a mail's menu | `s.read(message)` (#31); `s.write([add a document from the attachment])` |
| consume from outside | `on.outside(scanner)` | a scanner or phone app posts a file to the secret address; `s.write([add document])` |
| train | weekly | `s.job("classifier", documents)`; the model in `s.cache` |
| `rename by pattern` | `(pattern)` | `s.query`; `s.write(moves, { preview })` (#17, #32) |
| `search documents` | `on.call(query)` | `s.cache`, `s.query` |
| searchable PDF | after OCR | `s.job("pdf")`; a derived file in the cache (#33) |

## 7. Notes (`calternal-notes`)

Inherited:

- **Files and names.** Notes are flat files `Notes/<Title>.md` with an ID
  in the frontmatter (0023). A rename rewrites every link (G6).
- **Title** (0025): the first heading is the title, kept in step with
  `title:`, and the last change wins.
- **Timestamps:** `date` (created) and `last edited` (0025).
- **The text is the document** (0018), with rendered and source modes.
- **Properties** show as a table on notes that have any (Files
  decisions, 11-11).
- **Embedded collections.** An item made inside an embedded collection
  scoped to "this note" links back to that note (EM11-G (c)).
- **Daily notes** now belong to Calendar (0067, precedence fix 1).
- Primitives §8 adds both link forms, tags as a grouped collection, and
  snooze as work at a time.
- **Apple Notes** syncs over IMAP with UID identity (ADR 0069).

Editable outside.

```markdown
<!-- plugin.md -->
---
plugin: calternal-notes
name: Notes
version: 1.0.0
icon: note
format: 1
sends out: a server you name              # Apple Notes over IMAP (0069), only if connected
secrets: imap sign-in, per apple-notes-accounts
calls: []
---
Notes are Markdown files. Any editor can open them.
```

```markdown
<!-- kinds/notes.md -->
---
kind: notes
name: Note
folder: Notes
file: {title}                             # invented: #20
editable: outside
format: 1
fields:
  title: text, required, heading          # invented: #22
  tags: tags
  aliases: tags
  pinned: yes/no, default no
  snooze: time
  from: link(any)                         # where it was made from
  created: time, by host                  # invented: #23 (0025 `date`)
  edited: time, by host                   # 0025 `last edited`
row: title, edited
page: properties, body                    # the properties table only when it has any
search: title 3, aliases 3, tags 2, body
template: none                            # a person may name one (G10)
operations:
  add: title
  change
  remove
  pin: set pinned = not pinned, menu
  snooze: set snooze = ask, menu          # invented: #7
  duplicate                               # invented: #12
---
A note. The first heading is its title.
```

```markdown
<!-- collections: one block per file -->
---
collection: all-notes
title: Notes
from: notes
sort: pinned desc, edited desc
where: empty(snooze) or snooze <= now()
views:
  list: title, edited
---
collection: tags
title: Tags
from: notes
group: under(tags, "")                    # #9 (nested tags as groups)
total: count
views:
  list: title, edited
---
collection: linked-here
title: Linked here
from: notes, calternal-tasks.tasks, calternal-calendar.events   # needs #31
where: links to this
views:
  list: title
---
```

```markdown
<!-- sidebars/notes.md -->
---
sidebar: notes
title: Notes
---
- [Today](<calternal-calendar:days/today>)     # G14 (decided); the naming form is #49
- [Notes](<../collections/all-notes.md>)
- [Tags](<../collections/tags.md>)
- [Loose ends](<calternal-tasks:collections/loose-ends>)   # G14 (decided)
```

**Coded operations:**

| Operation | Signature | Calls |
| --- | --- | --- |
| resurface | `on.time(snooze)` | `s.notify([s.me], { item, text: "Back from snooze" })`; `s.write([change snooze = empty])` |
| Apple Notes sync | `on.connection(imap)` on an account item | `s.connect(account)`; `s.write([add or change notes by UID])`; deletion follows 0069 |

Notes is otherwise description-only: the editor is the catalogue's web code,
and rename, links, history and undo are the host's.

## 8. The iOS keyboard and the native Apple surfaces

The design (primitives §4) says native features are part of the native app
and store their data through plugin operations like any client. The
question here is whether the operations list and the fifteen calls are
enough for each surface, or what is missing.

| Surface | What it does | Expressible? | What is missing |
| --- | --- | --- | --- |
| **Synced clipboard** (keyboard) | Copy on one device, paste on another; the last weeks kept | Yes: a `clips` kind in the personal space, operations only, `add` and `remove`; the keyboard calls `add` and queries recent clips through the Rust client core | A stated retention (`keep: 30 days`, #36); without it the review checklist's item 7 fails |
| **Dictation** (keyboard, Superwhisper-like) | Speak, get cleaned text | Yes: the inherited voice decisions keep the original and a Markdown transcript; on-device recognition is native code; a server transcription is `s.job` | Nothing in the plugin API. Whether the model fits in a keyboard extension's memory is unverified |
| **Gesture typing that learns** | Swipe to type; learns your words | Yes: a `words` kind in the personal space (your words and how often you use them), synced like any kind | Nothing; the decoder is native code |
| **Share sheet** | "Share to Calternal" from any app | Partly: a shared URL, text or file can go to a kind's `add` | Which kinds accept what: a `share` flag on an `add` line (#38) |
| **Widgets** | A collection on the home screen; tick a task | Yes: a widget shows any collection's `compact` or `strip` view; an interactive tick runs a listed action (`done`), which applies on the device (S1) | Nothing in the plugin API; the widget picks a collection |
| **Menu bar** (macOS) | Today's events and tasks, quick add | Yes: the same as a widget, plus the composer | Nothing |
| **Notification actions** | "Done", "Snooze 1 hour", "Reply" on a notification | No: `s.notify(people, { item, text })` has no buttons | `actions:` naming operations-list actions, with one text input for Reply (#37) |
| **Shortcuts and App Intents** | "Add a task", "Log a set" from Shortcuts and Siri | Partly: the operations list is already one declaration for the app, CLI, API and MCP; Shortcuts can be a fifth surface | Apple compiles intents into the app, so plugins cannot add their own at run time. One generic intent per verb ("Add item", "Run action") with dynamic options is the likely shape (#45, unverified in detail) |
| **Live Activities** | A rest timer, an agent run's progress on the lock screen | No | A live progress surface for a running operation (#48); the rest timer itself counts on the device (#35) |
| **Apple Health** | Workouts written to Health | No: HealthKit is reachable only from native code on the device | A first-party device bridge, declared in the kind (`health: workout`) and done by the app, never by a plugin's code (#39) |
| **On-device photo library** | Back up the camera roll | Today through PhotoSync over WebDAV (FB9); later the native app uploads in the background | A device bridge (#39) |
| **Computer use** (macOS) | An agent drives the Mac | Yes: an agent run on the person's own machine (R5) | Nothing |

**Verdict:** the keyboard needs nothing new apart from a retention line. The
native surfaces need three things from the plugin API: notification
actions, share targets, and a rule for device bridges. Live Activities and
Shortcuts need a native design (question O2).

## 9. Serving CalDAV, CardDAV and WebDAV to outside clients

| Protocol | Who uses it | What it needs (inherited) | Can a plugin do it with the fifteen calls and six events? |
| --- | --- | --- | --- |
| **WebDAV** | Finder, Windows, PhotoSync, agents, the editable-outside mount | Decided as a host part: editable-outside kinds appear, operations-only kinds and `.calternal/` writes are refused (round 29), resumable uploads (F4, LQ3) | Not needed: it is the host's |
| **CalDAV** | Apple Calendar and Reminders, Thunderbird, DAVx5 | ADR 0067's "two-way CalDAV" appendix (on ADR 0064's base) | **No** (detail below) |
| **CardDAV** | Apple Contacts and other contact apps | ADR 0064: one writer, app passwords; ADR 0063: contact notes keep the original vCard lines | **No**, for the same reasons |
| **IMAP for Apple Mail** (the audit's "Apple Mail proxy") | Apple Mail reading Calternal's mirror | Not designed | **No**, for the same reasons |

What CalDAV needs, from ADR 0067's appendix and the ADR 0064 base it
follows:

- `/.well-known/caldav` and calendar-home discovery;
- separate VEVENT and VTODO collections;
- `PROPFIND`, plus `REPORT` for calendar-query, multiget and paged
  sync-collection, with a ctag fallback;
- ETags, 412 on a stale `If-Match`, and `If-None-Match: *`;
- Apple's header-less `MOVE` and `DELETE`;
- Basic sign-in with a scoped app password;
- every write as a durable plan, deletes to the 30-day Trash, and bounded
  recurrence expansion.

Why the fifteen calls cannot carry it:

- `on.outside(name)` is a webhook: one secret address, a JSON-style request
  to an operation. It has no methods, paths, status codes, XML bodies or
  Basic sign-in, and it cannot answer at the root `/.well-known/`.
- Waking a plugin process for every `PROPFIND` and `REPORT` would add the
  process's start time to every refresh Apple makes.
- Each plugin would rewrite the same tricky protocol and its Apple quirks.

The smallest addition is **host protocol adapters driven by declarations**
(#40, #41). The host serves CalDAV and CardDAV generically (in Rust, like
WebDAV) for any kind that declares a mapping: `ical: VEVENT` or
`ical: VTODO` with field lines, as in §1 and §2, or `vcard:` for contacts.
Writes from clients become ordinary writes by the signed-in person, so the
kind's checks and operations still apply. Detail C2.

## 10. What I had to invent

"Inherited" names the decision that already answers the need, if one does.
"For" says whether it goes to the owner (O1 to O4 in §13.1) or is a detail
for the coordinator (C1 to C5 in §13.2, the rest in §14). For a detail, the
smallest addition is the answer I recommend.

| # | Need | Apps | Smallest addition (the recommended answer) | Inherited | For |
| --- | --- | --- | --- | --- | --- |
| 1 | A line in a note is an item: a time line is an event, a checkbox is a task (or a plain checkbox), and the box mirrors the task's status | Calendar, Tasks | `lines: checkbox \| time in <kinds>` in a kind. The shared Rust core parses those two fixed line forms on the server and on devices, keeps the box and `status` in step, and lists plain checkboxes as `lines(checkbox)` | 0066, 0067, 0068 rules 1–4 settle the behaviour; not how a plugin declares it | Coordinator: **C1** |
| 2 | Field types the list lacks: `when` (a day or a time), `dates`, `durations`, `colour`, `number <unit>` | Calendar, Tasks, Workouts | Five words in the type table | 0068 (dates or timestamps, alert durations); 0067 (calendar colour) | Coordinator |
| 3 | A structured value only code changes (`source`: account, calendar, UID, href, ETag) | Calendar, Tasks | `data` type, shown read-only, changed only by the plugin's code | 0068 `source` is a non-secret map in frontmatter | Coordinator |
| 4 | Durable plugin data that is not a person's file and cannot be rebuilt: sync tokens, upstream ETags, the outbound queue, sent snapshots | Calendar (connected), Tasks (Reminders), Mail | A hidden operations-only kind (`shown: no`) the plugin writes with `s.write`; no new call | 0067 "durable outbound queue"; 0068 "retained wire snapshots belong in backed-up private state" | Coordinator |
| 5 | Repeats expanded inside a view's window, with per-day overrides and skipped dates | Calendar, Tasks Upcoming, Workouts | The engine expands items with `repeat:` in calendar and grouped views (`expand: repeat`); an item with `series` and `occurrence` replaces its slot, and `exceptions` removes one | 0067 CA15; 0068 repeats | Coordinator |
| 6 | Repeat after completion, after another event, or N times a period | Tasks | `next(repeat, after: date)`; goal progress counted from `log` | 0066, 0068 | Coordinator |
| 7 | A listed action that asks for one value (snooze until, schedule on) | Tasks, Notes | `set f = ask`: the field's own picker; still applies offline | 0066 drops and pickers | Coordinator |
| 8 | A condition inside a list function (`count(depends on, open)`), and `first()` | Tasks, Calendar | An optional condition argument, as `from()` has | — | Coordinator (fits F3) |
| 9 | Tag hierarchy: child tags of a parent | Tasks, Calendar, Notes | `under(tags, "area")`; `group: under(tags, "errands")` | 0066 group by tags under a parent; 0067 area tags | Coordinator |
| 10 | A sidebar entry for each item of a kind (each calendar, tag, person, type) | Calendar, Tasks, Photos, Documents | A sidebar line `- each: <kind>[, where c][, under "Heading"]` | 0066 tags in the sidebar; 0067 calendars | Coordinator |
| 11 | Plugin settings, also used in formulas | Tasks, Calendar, Workouts, Photos | `settings:` in `plugin.md`; `settings.x` in formulas | ADR 0055 (typed settings with stable IDs, scope, defaults) | Coordinator |
| 12 | Copy an item with a fresh identity | Tasks, Notes | A built-in `duplicate`, like add, change and remove | FB19 (copies get a new ID); 0066 `tasks.duplicate` | Coordinator |
| 13 | Manual order per person, list and group, outside the files | Tasks | `sort: manual`; the host keeps per-person ranks as backed-up app data | 0066 `order.db` | Coordinator |
| 14 | Dropping onto a list or group sets a field | Tasks | `drop: set f = v` in a collection | 0066 drops | Coordinator |
| 15 | A matrix of two groupings (Eisenhower) | Tasks | `matrix: rows, columns` view | 0066 Matrix | Coordinator |
| 16 | Composer phrases that fill an `add` form | Tasks, Calendar, Workouts | `phrases` on an `add` line; the phrase grammar is the host's language engine | 0066 Composer, 0053, L17 | Coordinator |
| 17 | A dry run shown before a bulk write commits | Tasks, Documents | `s.write(changes, { preview: true })` returns the plan for a confirm sheet | 0066 (dry run, preview, one-step undo); 0015 bulk jobs | Coordinator |
| 18 | A list of small records in frontmatter (a task's `log`) | Tasks, Calendar | `records(field: type, …)` as a field type | 0068 `log` | Coordinator |
| 19 | A streak over the log (skip does not break it, miss does) | Tasks habits | One engine function `streak(log)`, or drop the streak until F1 | 0066 streak rule | Coordinator |
| 20 | Folder and file names from fields (`Photos/2026/2026-11-27`, `Calendar/2026-11-27.md`, `Notes/<Title>.md`) | Calendar, Notes, Photos, Workouts, Documents | `folder:` and `file:` patterns; the host moves the file when the field changes and rewrites links (G6) | 0023, 0026, 0067, the attachments rule; PH26 is open for Documents | Coordinator (Documents: **C5**) |
| 21 | One item per value, and "open it, adding it if missing" | Calendar days, Workouts | `unique: f` and the listed form `open where …` | 0026 (opens on today's note) | Coordinator |
| 22 | The title kept in step with the first heading | Notes, Tasks, Calendar | `heading` flag on a text field | 0025 | Coordinator |
| 23 | Created and edited times every item has | Notes, Tasks, Files | Built-in read-only `created` and `edited` | 0025 | Coordinator |
| 24 | An item that is a binary file, with fields from its embedded metadata and fields in a `.calternal/` sidecar | Photos, Documents | `item: file(types)`, with `from metadata` and `in sidecar` field flags | FS1–FS3, PH27, PH-B, audit precedence fix 2 | Coordinator |
| 25 | Several files as one item (Live Photo, RAW+JPEG) | Photos | `paired` on a file item; removed and restored together | PH5, PR2, PR11 | Coordinator |
| 26 | Folders a person marks join a kind ("Show in Photos") | Photos | `folder: …, plus marked folders` | PR1, PH14 | Coordinator |
| 27 | Every file and folder as one source, with a folder view and shortcuts | Files | Built-in source `files` (name, folder, size, edited, type, kind), view `folders`, sidebar line `folders:` | F1, F2, FD1–FD5 | Coordinator |
| 28 | A kind-wide default filter (hidden photos left out unless asked) | Photos | `default where:` in a kind; `including hidden` in a collection | PH-A, PH-B | Coordinator |
| 29 | A plugin's own search (semantic, faces, learned) next to the host's | Photos, Documents | `search: <operation>` in `plugin.md`; the host merges ranked results under a time limit | PR8, F10 | Coordinator |
| 30 | A card or entry in another app's surface (memories in Home, Calendar's Today in Notes' sidebar) | Photos, Notes | `show in: home` on a collection; links to another plugin's collections and operations in a sidebar | — | Coordinator (extends G14 to Home) |
| 31 | Reading and listening to another plugin's kinds | Documents (mail), Tasks (events), Notes (linked here) | `reads: plugin.kind` in `plugin.md`, shown on the install sheet | — | Coordinator: **C4** |
| 32 | Move or rename a file from code | Photos, Documents, Calendar | A `move` change in `s.write`, rewriting links (G6) | 0023, FB19 | Coordinator |
| 33 | Large derived files from a job (searchable PDFs, transcodes) | Photos, Documents | `s.cache` files on the HDD tier, with a stated quota | PH7, PH8, ADR 0062 | Coordinator |
| 34 | Records across items: a collection, a chart and a formula over every session's sets, and record operations that apply offline | Workouts | `from: sessions.records` and `from(kind.records.field)`; `append record`, `change record` and `remove record` as listed operations | — | Coordinator: **C3** |
| 35 | A countdown on the device after an action (rest timer) | Workouts | `timer <duration>, after <action>` on an operation line; a catalogue component counting locally | — | Coordinator |
| 36 | Retention for a kind (the synced clipboard) | Keyboard | `keep: 30 days` in a kind | Review checklist item 7 | Coordinator |
| 37 | Buttons on a notification | Tasks, Calendar, Mail, Chat | `s.notify(…, { actions: [names] })`, with one text input | — | Owner: **O2** |
| 38 | Which kinds the share sheet offers | Notes, Documents, Files, Reading List | `share` on an `add` line | — | Owner: **O2** |
| 39 | Device bridges (Apple Health, the camera roll) | Workouts, Photos | A declaration in the kind that the first-party app carries out; never plugin code | FB9 for photos today | Owner: **O2** |
| 40 | Serving CalDAV and CardDAV (and IMAP to Apple Mail) | Calendar, Tasks, Contacts | Host protocol adapters, like WebDAV | 0064, 0067 appendix | Coordinator: **C2** |
| 41 | The iCalendar and vCard mapping of a kind | Calendar, Tasks, Contacts | `ical:` or `vcard:` lines in the kind | 0068 interop table; 0063 original vCard lines | Coordinator |
| 42 | The fields before a change, to compare against | Tasks, Calendar | `s.event.before` on `on.change` | 0066 reconciler; 0068 rule 4 | Coordinator |
| 43 | A formula with `now()` re-evaluated when its answer changes (planned → logged at the end time) | Calendar | The engine works out each such formula's next change time and re-runs it then, like the midnight re-run | 0067 (next boundary timer, no polling) | Coordinator |
| 44 | Contacts as a link target, though the suite has no Contacts app | Calendar, Tasks, Photos | `link(contacts)` with an optional `needs` on `calternal-contacts`; plain text when it is absent | 0063, 0064, 0068 (people are contact links) | Coordinator |
| 45 | Shortcuts and Siri for any operation | All | One generic intent per verb, with options read from the operations lists | — | Owner: **O2** |
| 46 | Partner sharing "everywhere": your partner's photos in your own timeline | Photos | A collection whose `from:` includes items shared with you, which reopens S4 | PR6 | Owner: **O1** |
| 47 | Where documents are filed, and renaming by pattern | Documents | `folder:` pattern (#20) chosen per space | PH26 (open) | Coordinator: **C5** |
| 48 | A live progress surface (lock screen, Dynamic Island) for a running operation | Workouts, Chat | A declared progress value an operation updates; the native app shows it | — | Owner: **O2** |
| 49 | One stable name for another plugin's kind, collection or operation | Notes, Tasks, Documents | `plugin:collections/name`, `plugin.kind`, `plugin.operation`, used alike in `needs`, `reads`, `adds to`, sidebars and `from` | — | Coordinator |
| 50 | A kind or folder that fewer people can read than the whole space (tax and medical papers in a family space) | Documents, Files | G5's `change:` line has a `read:` twin (`read: manager`, or named groups), enforced by the one check | FB20 (folder grants inherit, nothing widens) covers Files only | Owner: **O4** |

**Fifty inventions.** Forty-three are details for the coordinator, each
with the answer I recommend. Most have an inherited decision behind them,
and five of them (C1 to C5) are large enough to be argued in §13.2. Seven
rows (#37, #38, #39, #45, #46, #48 and #50) need the owner. They come down
to three questions (O1, O2 and O4), and O3 (Workouts' scope) comes from
Workouts having no decisions at all, not from a row.

## 11. How round 33's answers land in these apps

| Answer | Where it is used here | What is left |
| --- | --- | --- |
| **G5** `change: manager` | Documents could mark correspondents and types `change: manager` in a family space | Reading: no app may yet hide a kind from members who can read the space (#50, O4) |
| **G13** slots in `plugin.md` | Photos (face and OCR models, LQ2); Documents (OCR, classifier); Calendar (event describers); Tasks (reminder and interop providers) | Nothing |
| **G14** collections in another app's sidebar | Notes' sidebar shows Calendar's Today and Tasks' Loose ends | Home is not a sidebar: the "On this day" card needs G14 extended to Home (#30). I recommend treating Home as one more surface, with the same declaration and removal |
| **F1** `per month` with `previous()` | Workouts' weekly volume could be a per-week twin (`per week`), a one-word extension I recommend | The habit streak counts occurrences, not months, so it remains #19 (`streak(log)`, an engine function, as the coordinator's rule on engine functions says) |
| **F2** `totals:` with `all(kind)` | Not needed outside Money | Nothing |
| **F3** one hop | No app here needed two hops | Nothing |
| **F4** cap after the condition | Workouts (every bench set over years), Tasks (`time spent`), Documents (the serial-number check) | Nothing; these now work |
| **The engine as Wasm** | Lines (#1), repeat expansion (#5) and streaks run in the browser with the same code as the server and the phones | Nothing |
| **Coordinator: fields, operations and collections on another plugin's kind** | Documents adds "File to Documents" to Mail's messages (§6), instead of listening to every mail | Reading another plugin's items in code (#31) is the same pattern, and I recommend deciding it the same way (C4) |
| Operations on records inside an item | Workouts (sets), Tasks (`log`), Chat | Still open: C3 |

## 12. Verdict

**Calls and events, across all ten apps** (this test plus Money, Mail and
Chat):

| Call or event | Used by |
| --- | --- |
| `s.query`, `s.read`, `s.write`, `s.me`/`s.space`/`s.event` | Every app with code |
| `s.fetch` | Calendar (CalDAV client), Mail |
| `s.connect` | Mail, Notes (Apple Notes over IMAP) |
| `s.notify` | Notes (snooze), Chat |
| `s.job` | Photos, Documents |
| `s.cache` | Photos, Documents; Money only if F1 goes to code |
| `s.call` | Chat |
| `s.state` | Chat |
| `s.run`, `s.send`, `s.stop` | Chat only |
| `s.share` | Chat only; every other app shares through the host's Share sheet |
| `on.change` | Nearly every app |
| `on.schedule` | Calendar, Money |
| `on.time` | Notes |
| `on.call` | Photos, Documents (search), Chat |
| `on.outside` | Documents (scanner), automations |
| `on.connection` | Mail, Notes |

Every call and event is used at least once. The candidates to fold are the
calls only Chat uses:

- **`s.send` and `s.stop`** can be two methods of the handle that `s.run`
  returns, as the earlier test suggested. That takes the list from fifteen
  to thirteen.
- **`s.share`** stays, since sharing from code has no other route.

**Line forms that no app used:**

- **The `card` and `cell` keys.** Every view line already names its
  fields (`gallery: image`, `calendar: start`), so `row`, `card` and
  `cell` repeat what the views say. `row` is still the default list line
  and the line in search results, but `card` and `cell` can go.
- **The `timeline: a .. b` view.** No app used it: Photos' "timeline" is
  a grouped gallery, and Calendar uses `calendar: start .. ends`. The view
  can go, with `calendar` taking the range form.
- **A collection's member `fields`.** Nothing here needed member fields,
  though the format's trip board does. Keep it, since it is the only home
  for a board's own status.

**What I would simplify:**

- **Fold "item that is a file" (#24) into the folder item.** `item:
  folder with message.md` and `item: file(…)` are both "an item is not
  one Markdown file". One `item:` line with two forms covers Mail, Chat,
  Photos and Documents.
- **One way to name another plugin's thing.** #30 and #31 both need a
  stable name for another plugin's kind, collection or operation
  (`calternal-tasks:collections/loose-ends`). The format should define
  one form and use it everywhere: in `reads:`, `needs`, sidebars and
  `from`.
- **Most of Files is the host.** With #27, the Files plugin has no
  operations and no kinds. It could be the host's own default sidebar
  rather than a plugin.
- **Tasks and Calendar share one core feature, not two.** Lines (#1),
  repeat expansion (#5) and the log (#18) appear in both. Building them
  once in the shared core takes most of the code out of both plugins.

## 13. Questions

### 13.1 For the owner (high-level)

**O1. Partner sharing in Photos: may your timeline show items from another
space?**
*What:* PR6 asked for partner sharing of all photos, or those from a date,
shown in albums only or "everywhere". Decision S4 says collections never
cross spaces.
*Why now:* "everywhere" is impossible under S4 as decided.
*Example:* your partner shares their photos from 2024 on; do they appear in
your Library timeline?
*Options:*
(a) **A shared family space for photos**, which both libraries upload
into. Cost: photos move out of personal spaces, and quota is charged to
that space's owner.
(b) **One exception to S4:** a collection's `from:` may include items of
the same kind shared with you, read-only. Cost: queries span spaces, and
rights are checked per item.
(c) **Albums only:** shared albums, never the timeline. Cost: drops PR6's
"everywhere".
*Recommendation:* (b), limited to read-only items shared with you, since it
is what PR6 asked for and the rights check per item already exists.

**O2. What may native Apple surfaces do for plugins?**
*What:* notification buttons, share-sheet targets, Shortcuts, Live
Activities and Apple Health.
*Why now:* the plugin API must say what a plugin declares for them, and
Apple compiles some of them into the app.
*Example:* a task reminder with "Done" and "Snooze 1 hour"; a workout
written to Apple Health.
*Options:*
(a) **Derived generic surfaces**: notification `actions` from the
operations list, a `share` flag on `add`, one generic Shortcut per verb
with dynamic options, and a declared progress value for Live Activities.
Device bridges (Health, the camera roll) are first-party only and
declared in the kind. Cost: generic Shortcuts are less natural than
named ones.
(b) **Native features only for official plugins.** Cost: other plugins
feel second-class on iPhone.
(c) **Plugins ship native code.** Cost: no such code runs on devices
(ADR 0048 as changed), and App Store rules apply.
*Recommendation:* (a). The surfaces come from declarations that already
exist, so no plugin writes Apple code.

**O3. What does Workouts do in its first version?**
*What:* neither repository has any Workouts decisions. The reference apps
(RP Hypertrophy, MyoAdapt) set next week's sets and loads from your
feedback after each session.
*Why now:* it decides whether Workouts has coded operations at all.
*Example:* after a session you mark "pump: great, soreness: healed early";
next Monday's bench shows one more set.
*Options:*
(a) **Log only**: plans, sessions, sets, history and charts, and no
automatic progression. Cost: no coaching; description-only apart from
the rest timer.
(b) **Log plus a simple progression rule** from feedback (the shape in
§3). Cost: one coded operation, and the rule's numbers to decide with you.
(c) **Full coaching**, as in the reference apps. Cost: a research job into
their published methods first.
*Recommendation:* (b). It is what makes the reference apps worth using, at
the size of one operation.

**O4. May a kind or folder be readable by fewer people than its space?**
*What:* G5 lets a kind require a Manager to *change* its items. Nothing yet
lets a kind or folder be *read* by fewer people than the whole space.
*Why now:* it is a security limit, and Documents in a family space holds
tax, medical and salary papers.
*Example:* a family space where the children may see the photo albums but
not the parents' payslips.
*Options:*
(a) **A `read:` line as G5's twin** (`read: manager`, or named groups) on a
kind, and the inherited folder grants (FB20) for Files. Both are enforced
by the one check, and search, collections and formulas never show what the
person cannot read. Cost: a second rights rule; every query filters by it
(FB23 already requires filtering when the query is built).
(b) **Spaces are the only boundary**: put the private papers in a smaller
space. Cost: one app split across two spaces, and links between them are
cross-space (S4).
*Recommendation:* (a), since Files already has folder grants by
inheritance and one rule should serve both.

### 13.2 For the coordinator (detail, with the most versatile answer)

Each of these was written as a full question first; the recommendation is the
answer I would record under "Decided by the coordinator".

**C1. Are lines in a note items?**
*What:* in the old app, a line `- 09:00 - 10:00 Standup` in today's note is
an event, and `- [ ] Call the bank` with a link under it is a task whose box
follows the task's status everywhere (0066, 0067, 0068).
*Why now:* it decides whether this lives in the shared core (on every
device, offline) or in the Tasks and Calendar plugins' server code, and it
is the largest single piece of either app.
*Example:* on a plane you tick a task's box in today's note. Should Today in
Tasks show it done at once?
*Options:*
(a) **The shared core understands two fixed line forms**, a checkbox and a
leading time. A kind declares `lines:`, and the core parses the lines,
mirrors the box and lists plain checkboxes on the server and on devices.
Cost: a line parser and mirror rules in the core, about the size of the
formula engine's date handling. No other line forms are allowed later
without another decision.
(b) **The plugins reconcile on the server** with `on.change` and
`s.event.before`. Cost: an offline tick shows in the note but not in Tasks
until the phone reconnects, and each of the two plugins reimplements the
parsing.
(c) **Drop lines as items.** Every task and event is its own file, and a
line is only a link. Cost: this reverses accepted decisions and the
logging habit ("logging must be one tap", 0026).
*Recommendation:* (a). The behaviour is already decided in 0068. Only the
place it runs is new, and offline correctness is the reason to put it in
the core.

**C2. Who serves CalDAV and CardDAV to Apple Calendar, Reminders and
Contacts?**
*What:* outside clients must speak the protocol exactly: discovery,
`PROPFIND`/`REPORT`, ETags, sync tokens, app passwords and Apple's quirks
(0064, 0067).
*Why now:* the fifteen calls cannot express it (§9), and the old answer (a
Rust plugin, `calternaldav`) is gone with Rust plugins.
*Example:* you add an event on your iPhone in Apple Calendar; it should
appear in Calternal and in your daily note.
*Options:*
(a) **Host adapters, like WebDAV, driven by an `ical:` or `vcard:`
mapping in each kind.** Cost: CalDAV and CardDAV code in the host, and
the mapping lines in the format.
(b) **Plugins get raw HTTP routes** (methods, paths, headers, status
codes). Cost: every request wakes a plugin process; each plugin
reimplements the protocol; and plugins could answer at the server's root.
(c) **No serving**: Calternal only connects to other servers as a client.
Cost: Apple apps cannot see Calternal's own calendars or tasks, which
breaks 0067's Reminders sync.
*Recommendation:* (a). The protocol is written once, correctly, and every
plugin with a mapping gets it.

**C3. How do records inside an item work: sets in a workout, outcomes in a
task's log, messages in a chat?**
*What:* a workout session holds twenty sets. As separate files that is
7,000 files a year; as record lines in the session's note it is one
readable file.
*Why now:* it is still open from round 29, and Workouts cannot be written
without it.
*Example:* at the gym with no signal, you log "bench 80 kg × 8", then see
your best estimated one-rep max for the bench.
*Options:*
(a) **Record operations are listed** (`append record`, `change record`,
`remove record`) and apply on the device offline like other listed
operations. Collections and formulas may read records across items
(`from: sessions.records`). Cost: the engine indexes records as well as
items.
(b) **Records are changed only by code.** Cost: logging a set waits for
the server, so it fails at the gym.
(c) **Each record is its own item** (one file per set). Cost: thousands of
small files and a slower WebDAV folder.
*Recommendation:* (a). It also covers Chat's and Tasks' needs with one
rule.

**C4. May a plugin read another plugin's items?**
*What:* Documents wants mail attachments, Tasks wants time spent from
calendar events, and Notes' "Linked here" lists tasks and events.
*Why now:* nothing in the format says whether a plugin may query another's
kinds, and the install sheet must say so.
*Example:* installing Documents shows "Reads your mail to file
attachments".
*Options:*
(a) **`reads: plugin.kind` in `plugin.md`**, shown on the install sheet,
allows `s.query`, `s.read` and `on.change` on those kinds and nothing
else. Cost: one more line, and one more sentence on the sheet.
(b) **Only through the other plugin's listed operations** (`s.call`).
Cost: Mail must publish an operation for every use another plugin wants.
(c) **Any plugin reads every kind in the space.** Cost: a stranger's
habit tracker could read your mail.
*Recommendation:* (a). It is the same shape as `sends out:` and `calls:`,
and the person sees it before installing. It also follows the coordinator's 2026-11-27
decision that a plugin may add fields, operations and collections to
another plugin's kind: collections over another kind already read it, so
reading from code with a declaration grants nothing new in kind.

**C5. Where does Documents file things?**
*What:* PH26 asked why Documents is not by date like Photos, and was left
open. Paperless-ngx renames files by a pattern (correspondent, year,
title).
*Why now:* the kind's `folder:` line has to say.
*Example:* a gas bill from March 2026 lands in `Documents/2026/2026-03-14/`,
`Documents/Stadtwerke/2026/`, or wherever you dropped it.
*Options:*
(a) **By date** (the attachments rule), with renaming by pattern as an
explicit action. Cost: the folders mean little to a person browsing.
(b) **Where the person put it**, with collections doing the sorting.
Cost: inbox uploads need a default folder anyway.
(c) **A pattern per space** (`{correspondent}/{date:YYYY}`), with files
moved when fields change. Cost: files move under people's feet, which a
WebDAV user notices.
*Recommendation:* (a). It matches Photos and the inherited attachments
rule, and moving stays a deliberate action.

## 14. The other details for the coordinator

Rows 2 to 29, 32, 33, 35, 36, 41 to 44 and 49 of §10, each with its smallest
addition as the recommended answer and its inherited source. Three of them
deserve care:

- **#4 durable plugin data** is a hidden kind, not a new call. Its
  retention must be stated, because sent snapshots grow.
- **#20 file and folder patterns** move files. Every move goes through G6's
  link rewrite and shows in history.
- **#24 file items** keep XMP as the sidecar format. That is the inherited
  answer (PH27, PH-B), and a reason written down for not using Markdown,
  as AGENTS.md requires.

## 15. What could not be verified

- **Workouts and Documents behaviour** comes from general knowledge of the
  reference apps, not from their documentation, which was not read for
  this test.
- **Apple platform limits:** a keyboard extension's memory limit for
  on-device dictation, whether App Intents can take options read at run
  time, and widget refresh budgets were not checked against Apple's
  documentation.
- **The Files sharing round SH1–SH10** (upload-only drop links) was not
  re-read. The 2026-11-18 port (links with optional expiry and no password)
  was taken as the newer decision.
- **ADR 0064's full text** was not read. Its points here come from ADR
  0067's references to it and from the audit.
- **None of the costs** in O1 to O4 and C1 to C5 were measured.

## Simplification opportunities

- **Fold `s.send` and `s.stop` into `s.run`'s handle.** Fifteen calls
  become thirteen, and only Chat used either.
- **Drop `card` and `cell`**, and drop the `timeline` view in favour of
  `calendar: a .. b`. View lines already name their fields.
- **One `item:` line with two forms**, a folder with a main file or a
  binary file with a sidecar, for Mail, Chat, Photos and Documents.
- **Files as the host's default sidebar** instead of a plugin, since it
  has no kinds and no operations.
- **Lines, repeat expansion and the log** built once in the shared core
  for Tasks and Calendar (C1, #5, #18). That leaves both plugins with a
  few small coded operations each.
- **One naming form for another plugin's things**, used by `needs`,
  `reads`, `calls`, sidebars and `from`.
- **The format's example 2.12** (a Markdown file beside each scan)
  contradicts FS1 and should be replaced by the file item of §6.
