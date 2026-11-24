# Stress test: Money, Mail and Chat in the format · 2026-11-24

Status: **paper prototype**, no code. Money, Mail and Chat written out
completely in
[kind-and-collection-format-2026-11-24.md](kind-and-collection-format-2026-11-24.md):
every kind, collection, sidebar and `plugin.md`, and the signature of every
coded operation with the calls it makes (the call list is
[host-gaps §4.1](host-gaps-2026-11-24.md#step-4-running-code)). It assumes
the recommended answers of host-gaps 2.2 (one `operations` list per kind)
and 2.3 (`checks:`). Everything the three apps needed that the format or the
call list does not have is collected in §4; each use in the files is marked
`# invented: …`.

## 1. Money (`calternal-money`)

Envelope budgeting in the YNAB style (ADR 0057). Operations only.

```markdown
<!-- plugin.md -->
---
plugin: calternal-money
name: Money
version: 1.0.0
icon: money
format: 1
sends out: nothing
calls: []
---
Budgets, accounts and transactions. Imports bank files you give it; never
signs in to a bank.
```

```markdown
<!-- kinds/accounts.md -->
---
kind: accounts
name: Account
folder: Money/Accounts
editable: operations
format: 1
fields:
  name: text, required
  type: choice(Checking|Savings|Cash|Credit card)
  currency: text, default EUR
  closed: yes/no, default no
  balance: = sum(from(transactions.account).amount)
  cleared balance: = sum(from(transactions.account, cleared).amount)
  last reconciled: date
row: name, balance
create: name, type, currency
operations:
  add
  change: when not closed
  close: set closed = yes, when balance = 0
  reconcile: code
---
One file per account. Balances are computed from transactions; never type a
balance here.
```

```markdown
<!-- kinds/categories.md -->
---
kind: categories
name: Category
folder: Money/Categories
editable: operations
format: 1
fields:
  name: text, required
  group: choice(Bills|Needs|Wants|Savings)
  hidden: yes/no, default no
  assigned this month: = sum(from(assignments.category, month = month(today())).amount)
  spent this month: = sum(from(transactions.category, month(date) = month(today())).amount)
  available: code        # invented: a field whose value an operation computes into the cache
row: name, available
create: name, group
operations:
  add
  change
  hide: set hidden = yes, menu
---
```

```markdown
<!-- kinds/transactions.md -->
---
kind: transactions
name: Transaction
folder: Money/Transactions
editable: operations
format: 1
fields:
  date: date, default today
  payee: text, required
  amount: money, required
  account: link(accounts), required
  category: link(categories)
  memo: text
  cleared: yes/no, default no
  reconciled: yes/no, default no
  transfer: link(transactions)
  repeat: repeat
  import id: text
checks:
  amount != 0
  not (transfer and category)    # a transfer has no category
row: payee, amount, date
create: amount, payee, category, account, date
search: payee, memo
operations:
  add
  change: when not reconciled
  remove: when not reconciled
  clear: set cleared = yes, when not cleared, swipe
  transfer: code
  split: code
  import: code
---
One file per transaction, in the account's currency. Change reconciled
transactions only by undoing the reconciliation.
```

```markdown
<!-- kinds/assignments.md -->
---
kind: assignments
name: Assignment
folder: Money/Assignments
editable: operations
format: 1
fields:
  month: date, required          # the first day of the month
  category: link(categories), required
  amount: money, required
row: category, amount, month
operations:
  assign: code
  move: code
---
Money given to a category for a month. Use Assign and Move, not hand edits.
```

```markdown
<!-- collections/accounts.md -->
---
collection: accounts
title: Accounts
from: accounts
where: not closed
sort: type, name
total: sum(balance)
views:
  list: name, balance
---
```

```markdown
<!-- collections/budget.md -->
---
collection: budget
title: This month
from: categories
where: not hidden
group: group
total: sum(assigned this month), sum(spent this month)
views:
  table: name, assigned this month, spent this month, available
---
```

```markdown
<!-- collections/transactions.md -->
---
collection: transactions
title: All transactions
from: transactions
sort: date desc
views:
  table: date, payee, category, amount, cleared
  chart: sum(amount) by month(date)
---
```

```markdown
<!-- collections/uncleared.md -->
---
collection: uncleared
title: To clear
from: transactions
where: not cleared
sort: date desc
total: count, sum(amount)
views:
  list: payee, amount, date
---
```

```markdown
<!-- collections/scheduled.md -->
---
collection: scheduled
title: Scheduled
from: transactions
where: not empty(repeat)
sort: next(repeat)
views:
  list: payee, amount, next(repeat)    # invented: a formula as a view column without declaring it
---
```

```markdown
<!-- sidebars/money.md -->
---
sidebar: money
title: Money
---
- [This month](<../collections/budget.md>)
- [Accounts](<../collections/accounts.md>)
- [To clear](<../collections/uncleared.md>)
- [Scheduled](<../collections/scheduled.md>)
- [All transactions](<../collections/transactions.md>)
```

**Coded operations** (`operations.ts`):

| Operation | Signature | Calls |
| --- | --- | --- |
| `transfer` | `(from: account, to: account, amount: money, date: date)` | `s.write([add transaction −amount on from, add transaction +amount on to, each linking the other])`, one write that commits together |
| `split` | `(transaction, parts: {category, amount}[])` | `s.read(transaction)`; `s.write([change it to the first part, add the rest])`; refuses if the parts do not sum to the amount |
| `import` | `(account, file: file)` | `s.read(file)`; `s.query(transactions where account = … and import id in …)` to skip duplicates; `s.write([add …])` in batches of 1,000 |
| `reconcile` | `(account, statement balance: money, date)` | `s.query(transactions where account = … and cleared and not reconciled)`; `s.write([change reconciled = yes …, add an adjustment if the totals differ, change account last reconciled])` |
| `assign` | `(category, month, amount)` | `s.write([add or change the assignment])`; then recomputes `available` |
| `move` | `(from category, to category, month, amount)` | `s.write([change both assignments])` |
| `available` (cache) | `on.change(transactions, assignments)` | `s.query` per changed category and month, carrying each month's leftover forward; `s.cache.set(category, month, available)` |
| repeat | `on.schedule(repeat)` on transactions | `s.write([add the next occurrence])` |

## 2. Mail (`calternal-mail`)

ADR 0071 and the mail architecture plan: the server holds the IMAP
connection, the plugin fetches through it. Operations only.

```markdown
<!-- plugin.md -->
---
plugin: calternal-mail
name: Mail
version: 1.0.0
icon: mail
format: 1
sends out:
  - a server you name
secrets:
  - mail password, per account        # invented: a secret named per item, owned by the person who added it
calls:
  - calternal-files.add
---
Your mail accounts over IMAP and SMTP. The server signs in; Mail never sees
your password.
```

```markdown
<!-- kinds/mail-accounts.md -->
---
kind: mail-accounts
name: Mail account
folder: Mail/Accounts
editable: operations
format: 1
fields:
  name: text, required
  address: text, required
  imap: text, required        # host:port, a server you name
  smtp: text, required
  owner: person               # whose password it is
  last synced: time
row: name, address, last synced
create: name, address, imap, smtp
operations:
  connect: code
  remove: code
---
```

```markdown
<!-- kinds/mailboxes.md -->
---
kind: mailboxes
name: Mailbox
folder: Mail/Mailboxes
editable: operations
format: 1
fields:
  name: text, required
  account: link(mail-accounts), required
  role: choice(Inbox|Sent|Drafts|Archive|Trash|Junk|Other)
  unread: = count(from(messages.mailbox, unread))
row: name, unread
operations: []
---
Mirrors the server's folders. Created and removed by sync.
```

```markdown
<!-- kinds/messages.md -->
---
kind: messages
name: Message
folder: Mail/Messages
item: folder with message.md      # the raw message.eml sits beside it
editable: operations
format: 1
fields:
  subject: text
  from: text
  to: text
  cc: text
  date: time
  mailbox: link(mailboxes), required
  thread: link(threads)
  unread: yes/no, default yes
  flagged: yes/no, default no
  attachments: links(files)
  message id: text
row: from, subject, date
page: web(body)                   # invented: a look that draws the HTML part in a web view
search: subject, from, to, body
operations:
  change: only unread, flagged    # invented: change limited to listed fields
  archive: code
  move: code
  delete: code
---
One folder per message: `message.md` holds the headers and the text part;
`message.eml` is the original, untouched.
```

```markdown
<!-- kinds/threads.md -->
---
kind: threads
name: Conversation
folder: Mail/Threads
editable: operations
format: 1
fields:
  subject: text
  latest: = max(from(messages.thread).date)
  count: = count(from(messages.thread))
  unread: = count(from(messages.thread, unread))
row: subject, latest, unread
page: records(from(messages.thread))   # invented: a records view over items of another kind
operations: []
---
```

```markdown
<!-- kinds/drafts.md -->
---
kind: drafts
name: Draft
folder: Mail/Drafts
editable: outside
format: 1
fields:
  to: text, required
  cc: text
  subject: text
  account: link(mail-accounts), required
  reply to: link(messages)
  send at: time
row: subject, to
create: to, subject, account
operations:
  add
  change
  remove
  send: code
---
Write the body below the frontmatter. Sending turns the draft into a message
in Sent.
```

```markdown
<!-- collections/inbox.md -->
---
collection: inbox
title: Inbox
from: threads
where: latest message in a mailbox with role Inbox   # invented: a condition across a link of a link
sort: latest desc
views:
  list: subject, latest, unread
---
```

The Inbox needed a second hop (thread → messages → mailbox role), which the
one-hop limit forbids. Written within the limit, Inbox lists messages, not
threads:

```markdown
<!-- collections/inbox.md (as allowed) -->
---
collection: inbox
title: Inbox
from: messages
where: mailbox.role = "Inbox"
sort: date desc
group: thread
views:
  list: from, subject, date
---
```

```markdown
<!-- collections/unread.md -->
---
collection: unread
title: Unread
from: messages
where: unread
sort: date desc
total: count
views:
  list: from, subject, date
---
```

```markdown
<!-- collections/flagged.md -->
---
collection: flagged
title: Flagged
from: messages
where: flagged
sort: date desc
views:
  list: from, subject, date
---
```

```markdown
<!-- collections/drafts.md -->
---
collection: drafts
title: Drafts
from: drafts
sort: updated desc
views:
  list: subject, to
---
```

```markdown
<!-- collections/mailboxes.md -->
---
collection: mailboxes
title: Mailboxes
from: mailboxes
group: account
views:
  compact: name, unread
---
```

```markdown
<!-- sidebars/mail.md -->
---
sidebar: mail
title: Mail
---
- [Inbox](<../collections/inbox.md>)
- [Unread](<../collections/unread.md>)
- [Flagged](<../collections/flagged.md>)
- [Drafts](<../collections/drafts.md>)
- [Mailboxes](<../collections/mailboxes.md>)
```

**Coded operations:**

| Operation | Signature | Calls |
| --- | --- | --- |
| `connect` | `(account)` | the server's sheet confirms the named servers with the member (round 23); the server opens IMAP with the account's secret; `s.write([change last synced])` |
| sync | `on.connection(mail-accounts)` with new or changed UIDs | `s.connect(account)` to fetch headers and bodies; `s.write([add or change messages, threads, mailboxes])` in batches; attachments through `s.call("calternal-files.add", …)` |
| `archive`, `move`, `delete` | `(message[, mailbox])` | `s.connect(account)` for IMAP MOVE or EXPUNGE; then `s.write([change mailbox])` or `remove` |
| `send` | `(draft)` | `s.connect(account's SMTP)`; on success `s.write([remove draft, add message in Sent])`; with `send at` it runs at that time (`on.time(send at)`) |
| `remove` (account) | `(account)` | `s.write([remove its messages, threads, mailboxes, the account])` in batches; the server forgets the secret |

## 3. Chat (`calternal-chat` and its parts)

One record file per chat; a chat between people lives in the starter's
personal space and is shared (T8). Operations only.

```markdown
<!-- calternal-chat/plugin.md -->
---
plugin: calternal-chat
name: Chat
version: 2.0.0
icon: chat
format: 1
sends out: nothing
calls: []
---
Conversations with people and agents.
```

```markdown
<!-- calternal-chat/kinds/chats.md -->
---
kind: chats
name: Chat
folder: Chats
item: folder with chat.md         # attachments of the chat beside it
editable: operations
format: 1
fields:
  title: text
  people: people, required
  archived: yes/no, default no
  agents in: = count(from(runs.chat, running))   # needs the harness's runs kind
records:
  text: long text
  quote: link(chats records)      # invented: a link to a record of the same kind
  mentions: people
  attachments: links(files)
state:                            # invented: declared per-person state keys
  read: record                    # the read position
  muted: yes/no
  draft: long text
unread: = count(records after state.read)   # invented: formulas over records and state
row: [title, people], unread
page: records
checks:
  records.text != "" or records.attachments   # invented: checks over records
operations:
  add
  change: only title, archived
  send: append record
  edit: change record, by author, when now() - record.at <= 48h
  delete: remove record, by author or manager
  mute: set state.muted = yes, menu
  read: set state.read = record      # sent by the client as you scroll
  share: code
---
```

```markdown
<!-- calternal-chat/collections/pinned.md -->
---
collection: pinned
title: Pinned
views:
  strip: [title, people]
---
```

```markdown
<!-- calternal-chat/collections/groups.md -->
---
collection: groups
title: Groups
from: chats
where: count(people) > 2 and not archived
sort: updated desc
views:
  list: [title, people], unread
---
```

```markdown
<!-- calternal-chat/collections/people.md -->
---
collection: people
title: People
from: chats
where: count(people) = 2 and not archived
sort: updated desc
views:
  list: [title, people], unread
---
```

```markdown
<!-- calternal-chat/collections/archived.md -->
---
collection: archived
title: Archived
from: chats
where: archived
sort: updated desc
views:
  compact: [title, people]
---
```

```markdown
<!-- calternal-chat/sidebars/chat.md -->
---
sidebar: chat
title: Chat
---
- [Pinned](<../collections/pinned.md>)
- [Groups](<../collections/groups.md>)
- [People](<../collections/people.md>)
- [Archived](<../collections/archived.md>)
```

The **Harnesses** section comes from `calternal-chat-ai`, which adds a
collection to the Chat sidebar's default order (item 19 in §4). The **Cals**
section needs a `cals` kind, which belongs to the Cal plugin and is not
written here.

```markdown
<!-- calternal-chat-ai/plugin.md -->
---
plugin: calternal-chat-ai
name: Chat agents
version: 2.0.0
icon: spark
format: 1
needs:
  - calternal-chat ^2
sends out: nothing
calls:
  - calternal-chat.send
slots:                             # invented: a slot declaration in plugin.md
  harness: others, recommended calternal-chat-claude-code
---
Ask an agent in a chat.
```

```markdown
<!-- calternal-chat-ai/kinds/runs.md -->
---
kind: runs
name: Agent run
folder: Chats/Runs
item: folder with run.md          # the run's log and inbox beside it
editable: operations
format: 1
fields:
  chat: link(chats), required
  harness: text
  started by: person
  state: choice(Waiting|Running|Stopped|Done|Failed)
  running: = state = "Running"
row: harness, state, started by
operations:
  call: code
  steer: code
  stop: code
---
```

```markdown
<!-- calternal-chat-ai/collections/harnesses.md -->
---
collection: harnesses
title: Harnesses
from: chats
where: count(from(runs.chat)) > 0     # chats an agent was called into
sort: updated desc
views:
  list: [title, people], unread
---
```

`calternal-chat-composer` (the composer slot's default part) and
`calternal-chat-notify` are a `plugin.md` each plus code; a harness such as
`calternal-chat-claude-code` is a `plugin.md` with `programs:` and
`sends out: - api.anthropic.com`.

**Coded operations:**

| Plugin | Operation | Signature | Calls |
| --- | --- | --- | --- |
| chat | `share` | `(chat, people)` | `s.share(chat, people)` (invented: a host sharing call, missing from the call list) |
| chat-composer | `send` | slot operation: `(text, quote?, mentions?, attachments?)`, target filled by the host | `s.call("calternal-chat.send", …)` |
| chat-ai | `call` | `(chat, prompt, scope? = session)` | the host resolves the harness from the pick; `s.run(program, { scope, targets: [chat] })` returns after the scope is fixed; then `s.call("calternal-chat.send", { text: prompt })`; `s.write([add a runs item])` |
| chat-ai | `steer` | `(run, text)` | refused unless the caller may write in the chat (A18); `s.run…send(text)` (invented: sending input to a run is not in the call list) |
| chat-ai | `stop` | `(run)` | `s.run…stop()` (same gap); `s.write([change state = Stopped])` |
| chat-ai | reply | the run's fenced call into `calternal-chat.send`, as the run | — |
| chat-notify | notify | `on.change(chats)` | `s.read(chat records after the event's cursor)`; `s.notify(mentioned people, { item, text })`; the host skips muted people |
| chat | session scope | — | "the current chat session" (records after a quiet hour) is a scope the host computes; it has no formula (invented: `session()`) |

## 4. What I had to invent, or could not express

| # | Need | Where | Proposal |
| --- | --- | --- | --- |
| 1 | A field whose value a coded operation computes and the cache holds (`available` with month-to-month carryover) | Money categories | `field: code`: computed by an operation into the rebuildable cache, shown like a formula; formulas cannot express a running total over time |
| 2 | A formula as a view column without declaring a field (`next(repeat)`) | Money scheduled | Allow any formula in a view line |
| 3 | A total over all items of a kind, not linked ("Ready to Assign") | Money | Not expressed: needs `sum(all(kind).field)` or a budget-level item; I left it to the `available` cache |
| 4 | A secret per item, owned by the person who added it | Mail accounts | `secrets: name, per <kind>` in `plugin.md` |
| 5 | A change limited to listed fields | Mail messages, Chat | `change: only a, b` in the operations list |
| 6 | A page drawing an HTML body in a web view | Mail messages | `page: web(field)` |
| 7 | A records view over items of another kind (a mail thread) | Mail threads | `page: records(from(kind.field))`, the drawing spike's records component over several files |
| 8 | A condition across two hops (thread → message → mailbox role) | Mail Inbox | Not expressed within one hop; Inbox lists messages grouped by thread instead |
| 9 | A link to a record of a kind | Chat quotes | `link(<kind> records)` |
| 10 | Declared per-person state keys | Chat | `state:` section in a kind; `set state.x = …` in the operations list |
| 11 | Formulas over records and state (`count(records after state.read)`) | Chat unread | Allow `records` and `state.` in kind formulas |
| 12 | Checks over records | Chat | `records.<field>` in `checks:` |
| 13 | Record operations in the operations list (`append record`, `change record, by author, when …`, `remove record, by author or manager`) | Chat | Part of host-gaps 2.2's one list |
| 14 | A slot declaration in `plugin.md` (`harness: others, recommended …`) | Chat AI | `slots:` key in `plugin.md` |
| 15 | Sending input to a run and stopping it | Chat AI | `s.run(…)` returns a handle with `send` and `stop`; add both to the call list |
| 16 | Sharing an item with people | Chat | `s.share(item, people)`, missing from the call list |
| 17 | "The current chat session" | Chat AI | A host-computed scope, `session()`; formulas do not need it |
| 18 | An item that is a folder with its main file and siblings | Mail, Chat, runs | `item: folder with <file>` (the skills box's form), used by three kinds here |
| 19 | One plugin adding collections to another plugin's default sidebar | Chat AI | `sidebars:` in the adding plugin naming the target sidebar and position |
| 20 | Currencies: `money` with an account's currency | Money | Left as `money` without a currency; totals across currencies wait (round 26) |

**What held without invention:** multi-change writes (transfer, split,
reconcile), one-hop rollups (balances, unread per mailbox, thread counts),
the repeat field, totals by one field, checks in the formula language,
operations-only kinds with listed built-ins, server-held connections for
mail, the composer as a slot operation with its target filled by the host,
and agent runs through `s.run`.

**Size:** Money is 5 kinds, 5 collections, 1 sidebar and 8 coded operations;
Mail 5 kinds, 5 collections, 1 sidebar and 7 coded operations; Chat 2 kinds
across 4 plugins, 5 collections, 1 sidebar and 6 coded operations, plus the
run's fenced reply and the host-computed session scope.
