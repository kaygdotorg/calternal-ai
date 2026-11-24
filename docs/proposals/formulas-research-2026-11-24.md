# Where formulas stop: Actual Budget, YNAB, Notion and others · 2026-11-24

Open question G11 (morning report, thirty-first round): the owner asked for
research into how Actual Budget, YNAB and Notion compute their numbers
before deciding where our formula language stops. This document reports what
each product does and what a user may write there. It then puts our three
hard cases (from
[the stress test](format-stress-test-money-mail-chat-2026-11-24.md), table
in §5) through three designs each, and ends with questions for the owner.

Our formula language as it stands is in
[the format](kind-and-collection-format-2026-11-24.md) §1.6: sixteen
functions, one link hop, at most 1,000 items per `from()` or link list, a
step budget, no writes, and values kept only in the rebuildable cache. The
same Rust library computes formulas on the server and on devices (round 29,
S3). Plugin code runs only on the server.

## Summary

- **Actual Budget does not use a formula language for its budget.** The
  budget is plain code that builds a spreadsheet of named cells, one sheet
  per month. Each month's cells read the previous month's cells. That one
  idea gives both carryover and "To Budget". Formulas (HyperFormula,
  Excel style) are an experimental extra, used only in dashboard cards and
  rule actions. Budget templates (`#template 50 up to 300`) are a small
  separate language that *writes* budget amounts when you press a button.
  They do not compute fields.
- **YNAB lets a user write no formulas at all.** Ready to Assign, category
  balances, targets and Age of Money are computed by YNAB. The public API
  hands them out as computed numbers for each month.
- **Notion has the richest language**: lists, `map`/`filter`/`every`, `let`,
  and relations followed to any depth. Even so it cannot do a running total
  over rows or carry a value from one month to the next. It also forbids a
  rollup of a rollup "as this could create unintended loops".
- **What this means for our cases:**
  1. *Carryover* (Money "available") is a value that depends on the same
     value one month earlier. No product does this with user-written
     formulas. Actual does it with per-month cells. A small addition would
     let our formulas do it too: fields computed **per month**, plus
     `previous(field)`.
  2. *Ready to Assign* is a total over every category. Categories are a
     small kind, so a **total over all items of a kind** fits inside the
     1,000 cap. The total also carries from month to month, so it needs
     item 1 as well.
  3. *Mail Inbox* (two hops) **already fits the current language**. A
     message gets a computed field `in inbox` (one hop to its mailbox), and
     the thread counts its messages with that field (one hop). This is
     Airtable's way: one field per hop. Row 8 of the stress test was wrong
     to call it inexpressible.
- **Recommendation:** grow the language by two constructs: per-month fields
  with `previous()`, and totals over all items of a small kind. Do not add
  fields "computed by code". Keep one hop. The main reason is offline use.
  Plugin code never runs on a device, so a code-computed "available" would
  not move on a phone in a plane, the very place where Actual and YNAB show
  the right number.

## 1. How this was researched

| Source | What was read |
| --- | --- |
| Actual Budget source, `github.com/actualbudget/actual` at commit `258dcab21006784c4419768f7601b92eae43d767` (2026-11-23) | Cloned on `calternal-dev-01-compute02` under the lane directory, read with grep and sed; nothing was run |
| Actual's own docs, in the same clone, `packages/docs/docs/` | `experimental/formulas.md`, `experimental/goal-templates.md` |
| YNAB | Public API page <https://api.ynab.com/> and its OpenAPI file <https://api.ynab.com/papi/open_api_spec.yaml> (version 1.92.0); help pages listed in §3 |
| Notion | Help pages <https://www.notion.com/help/formulas>, <https://www.notion.com/help/formula-syntax>, <https://www.notion.com/help/relations-and-rollups>; API page <https://developers.notion.com/reference/page-property-values> |
| Airtable, Obsidian Bases, Coda | Pages listed in §4 |

Firecrawl was used for web pages. Context7 was not needed.

## 2. Actual Budget

### 2.1 How the budget is computed

All paths are under `packages/loot-core/src/server/` at the commit above.

- **A home-made spreadsheet engine** (`spreadsheet/spreadsheet.ts`, with
  `spreadsheet/graph-data-structure.ts`). Cells have names like
  `budget202611!leftover-<category id>`. A cell is static (a stored
  value), dynamic (`createDynamic({ dependencies, run })`: a plain
  TypeScript function of named cells), or a query (an AQL query compiled to
  SQL, `server/aql/`). Each change runs in a transaction that collects the
  dirty cells, sorts them topologically along the dependency graph, and
  recomputes them in that order. Only the cells that depend on the change
  are recomputed.
- **One sheet per month** (`budget/base.ts`, `getBudgetRange`). Sheets
  exist from 3 months before the oldest transaction to at least 12 months
  ahead (`MIN_MONTHS_AHEAD = 12`). The end moves up to 3 months past the
  last month with money assigned, but never past 60 months ahead
  (`MONTHS_AFTER_LATEST_ALLOCATION = 3`, `MAX_MONTHS_AHEAD = 60`).
- **Carryover is a reference to last month** (`budget/envelope.ts`, lines
  46–66). For each category:

  ```
  leftover = budgeted + spent
           + (last month's carryover flag ? last month's leftover
                                          : last month's leftover if positive, else 0)
  ```

  `carryover-<id>` is a stored yes/no for each category and month. Without
  it, an overspent category starts the next month at zero, and the
  overspending is charged to To Budget instead.
- **"To Budget" is a chain of summary cells per month** (`envelope.ts`,
  lines 103–212):

  ```
  from-last-month      = last month's to-budget + last month's buffered
  available-funds      = total income + from-last-month
  last-month-overspent = sum over expense categories without carryover of min(0, last month's leftover)
  to-budget            = available-funds + last-month-overspent + total-budgeted − buffered
  ```

  (`total-budgeted` is stored as a negative number.) "Buffered" is money
  the person holds back for next month.
- **Spending per category per month is one SQL sum** (`budget/base.ts`):
  the cell `sum-amount-<category>` runs `SELECT SUM(amount)` over that
  month's on-budget transactions in the category. On a cold start all of
  these are filled from one grouped query (`getSumAmountsByMonth`), not
  one query per cell.
- **One transaction change recomputes little.** `handleTransactionChange`
  (`base.ts`, from line 220) acts only when the date, account, amount,
  category, deleted flag or split parent changed. It recomputes only the
  `sum-amount` cell for that month and category, and for the old month or
  category too when one of them moved. The graph then updates that
  category's leftover for this and every later month, and the month
  summaries.
- **Sync and offline.** Actual is local-first. Each device keeps the whole
  budget in SQLite and syncs changes to rows (CRDT messages with a Merkle
  clock, `sync/index.ts`). **Computed values are never synced.** After a
  batch of remote changes is applied, the device recomputes the affected
  cells itself (`triggerBudgetChanges` and `triggerDatabaseChanges`, inside
  one sheet transaction, around line 578). Values are cached on disk in a
  `kvcache` table, keyed so a stale cache is thrown away (`sheet.ts`); the
  browser build keeps no such cache.
- **Two budget types**: envelope (`zero_budgets`) and tracking
  (`reflect_budgets`, `budget/tracking.ts`). They are the same engine with
  different cells.
- **Reports are code too.** Age of Money, cash flow and custom reports are
  TypeScript in `packages/desktop-client/src/components/reports/spreadsheets/`
  (for example `age-of-money-spreadsheet.ts`, which follows income
  "buckets" first in, first out).

### 2.2 What a user may write

| Feature | Language | What it does | Source |
| --- | --- | --- | --- |
| Rule formulas (experimental) | Excel style through HyperFormula; must start with `=`; transaction fields are named values (`=IF(amount<0, "Expense", "Income")`) | Sets one field of a transaction when a rule fires | `server/rules/action.ts`; `docs/experimental/formulas.md` |
| Formula cards (experimental) | Same, plus Actual's own functions: `QUERY`, `QUERY_COUNT`, `BUDGET_QUERY("spent", categories, from, to)`, `BALANCE_OF(account)`, `INTEGER_TO_AMOUNT`, `FORMATCURRENCY`… | Shows one number on the reports dashboard | `shared/formulas/customFunctions.ts`; `desktop-client/src/hooks/useFormulaExecution.ts` |
| Rule templates | Handlebars text templates | Builds text such as a note or payee from fields | `docs/experimental/rule-templating.md` (title only read) |
| Budget templates | A small grammar of note lines (`budget/goal-template.pegjs`, 118 lines, peggy): `#template 50`, `#template 50 up to 300`, `#template up to 150 hold`, `#template 500 by 2027-03 repeat every year`, `#template 10 repeat every 2 weeks starting 2025-01-04`, percentages of income, `remainder`, `schedule`, priorities `#template-2` | When the person chooses *Apply* or *Overwrite with budget template*, writes the month's budgeted amounts. A goal ring shows progress. | `docs/experimental/goal-templates.md`; `budget/template-notes.ts` |

**The pattern:** the numbers that must always be right (balances, carryover,
To Budget) are computed by code inside a dependency graph. User-written
formulas only fill a field once (rules), show a number (cards), or produce
amounts that a button writes (templates). No user formula feeds the core
budget.

### 2.3 Limits

- HyperFormula is GPL-licensed and runs once per formula evaluation, with
  the transaction's fields bound as names (`rules/action.ts`). There is no
  documented step or row limit; whether `QUERY` has a row cap was not
  checked.
- The engine has no general cycle check for user input, because users never
  write cells. Its cells are fixed by code.

## 3. YNAB

- **No user formulas.** Help pages describe what YNAB computes; nothing in
  them lets a person write an expression.
- **Ready to Assign** "holds the money that still needs a category". Money
  enters it through account balances and inflows. It can be negative in the
  current month, or zero now but negative in a future month when the
  person has assigned future money they do not yet have
  (<https://support.ynab.com/en_us/assigning-your-money-a-guide-SypgkrNJi>).
  "When the month rolls over, cash overspending (red) in the previous month
  reduces Ready to Assign"
  (<https://support.ynab.com/en_us/when-ready-to-assign-is-negative-an-overview-HylZA0zCc>).
  This is the same rule as Actual's non-carryover categories.
- **Targets** have behaviours: "Set aside another…" asks for the full
  amount again each period, "Refill up to…" asks only for what was spent,
  and "Have a balance of…" is not meant to be spent from before the date
  (<https://support.ynab.com/en_us/how-to-use-targets-rk5kkI9ks>). A target
  that is short shows the category in yellow.
- **Age of Money** "considers your last ten outflow cash transactions
  (including credit card payments)" and averages how long that money sat
  in the accounts. Transfers and starting balances do not count
  (<https://support.ynab.com/en_us/age-of-money-H1ZS84W1s>).
- **The API returns computed numbers per month** (OpenAPI 1.92.0):
  - for each month: `income`, `budgeted`, `activity`, `to_be_budgeted`
    (Ready to Assign) and `age_of_money`;
  - for each category in a month: `budgeted`, `activity`, `balance`, the
    target's settings (`goal_type`, `goal_cadence`, `goal_target`,
    `goal_target_month`, …), and computed progress (`goal_percentage_complete`,
    `goal_months_to_budget`, `goal_under_funded`, `goal_overall_funded`,
    `goal_overall_left`).

  The API is "mostly read-only", allows 200 requests per hour per token,
  and supports delta requests (`server_knowledge`).

**The pattern:** everything is computed by the product for each month and
exposed as plain numbers. The month is the unit, as in Actual.

## 4. Notion, and briefly Airtable, Obsidian Bases and Coda

### 4.1 Notion formulas (2.0)

- **Types**: text, number, yes/no, date, person, page, and lists of these
  (<https://www.notion.com/help/formula-syntax>).
- **Functions**: about 90, among them `if`/`ifs`, text with regular
  expressions (`test`, `match`, `replaceAll`), maths, dates (`dateAdd`,
  `dateBetween`, `formatDate`), and lists (`at`, `first`, `last`, `slice`,
  `sort`, `unique`, `includes`, `find`, `filter`, `some`, `every`, `map`,
  `flat`, `sum`, `mean`, `median`). `let` and `lets` name values, and
  `style` colours text.
- **Relations to any depth**: a relation is a list of pages, and a page's
  properties can be read in turn: `prop("Tasks").filter(current.prop("Status") !== "Done")`.
  An automation example on the help page goes two hops,
  `Parent Task.Sub-item.every(current.Status == "Done")`
  (<https://www.notion.com/help/formulas>).
- **Rollups** apply one calculation (count, sum, average, min, max, range,
  percent empty, earliest date…) to one property across related pages. "Can
  I rollup a rollup? Unfortunately not, as this could create unintended
  loops." Column totals at the foot of a table are display only
  (<https://www.notion.com/help/relations-and-rollups>).
- **Formulas cannot write.** Writing is done by buttons and automations,
  which use formulas to compute the values they set.
- **No previous row, no running total.** The known workaround relates each
  row to every earlier row and sums the relation
  (<https://bensomething.notion.site/Cumulative-Sum-6c759e42a4b948cfafed9f3fbbc0c495>,
  a community page, not Notion's). The work grows with the square of the
  row count.
- **Limits:** the pages read state no depth limit and no row limit for
  formulas. The API returns at most 25 relation references per page
  response and pages through the rest
  (<https://developers.notion.com/reference/page-property-values>); this is
  an API limit, not a formula limit.

### 4.2 Airtable

Rollups aggregate one field across linked records with an aggregation
formula over `values` (`SUM(values)`, `ARRAYUNIQUE(values)`…), and can be
limited to linked records that meet conditions. Several hops are built as
**one field per hop**: the `ARRAYFLATTEN` row says "use when you have a
lookup of a lookup" (<https://support.airtable.com/docs/rollup-field-overview>,
<https://support.airtable.com/docs/lookup-field-overview>).

### 4.3 Obsidian Bases

A base is a YAML file with `filters`, `formulas`, `views` and `summaries`.
Formulas may use other formulas "as long as there's no circular reference".
Summary formulas see `values`, the list of a property's values across the
view (`values.mean().round(3)`): a total over every row, for display.
`link.asFile()` returns a file whose `file.properties` can be read, so
links can be followed hop after hop. Lists have `filter`, `map` and
`reduce` (<https://help.obsidian.md/bases/syntax>,
<https://help.obsidian.md/bases/functions>). The page says plugins will
later be able to add functions.

### 4.4 Coda

Coda's help pages have moved, since Coda is now part of Superhuman, and
the formula page could not be read. Community answers give the usual
running total as `thisTable.Filter(RowId() <= thisRow.RowId()).Amount.Sum()`
(<https://connect.superhuman.com/t/adding-values-from-previous-rows/729>).
This is again a query over all earlier rows for every row.

## 5. What the products teach

| Question | Actual | YNAB | Notion | Airtable / Bases |
| --- | --- | --- | --- | --- |
| Can a user write the budget's maths? | No; code cells | No | Yes, but there is no budget | Yes, no budget |
| Month-to-month carry | A cell that reads last month's cell | Computed by YNAB | Not possible | Not possible |
| Total over every item | Summary cells per month | Per-month numbers in the API | Footer totals (display only), or a row related to all | Bases: view summaries (display only) |
| Two hops | n/a | n/a | Any depth inline | One field per hop (Airtable); any depth (Bases) |
| Where it runs offline | On every device; computed values never synced | Not checked | Not checked | Bases: locally, on files |

Two lessons matter most:

1. **The month is the unit.** Both budgeting products compute a grid of
   category × month, and each month reads the one before. Row-based
   products (Notion, Coda) can only imitate this with quadratic
   workarounds.
2. **Computed values are never the record.** Actual syncs only the rows a
   person changed and recomputes on each device. That is exactly our rule
   (values only in the rebuildable cache, S3).

## 6. Case 1: Money "available", carried month to month

The need: a category's available amount this month is what was assigned
plus what was spent (spending is negative) plus what was left last month.
Overspending is either carried, or dropped and charged to Ready to Assign.

### (a) Formulas unchanged; the field is "computed by code"

```markdown
available: code
```

An operation in `operations.ts` listens for changes to transactions and
assignments, recomputes the months that changed, and writes the values into
the cache.

- **Host cost:** a new call for code to write cache values, and a rule for
  when such values are stale.
- **Plugin cost:** the author rewrites Actual's
  `handleTransactionChange`: old and new month, old and new category,
  deleted transactions, transfers, then every later month. Each budgeting
  plugin writes this again.
- **Offline cost:** plugin code never runs on a device (ADR 0048 as
  changed, owner 2026-11-19). On a plane, a new transaction does **not**
  change "available" until the phone reconnects. That breaks S3 for
  Money's main number. Actual updates at once, offline.

### (b) A small growth: fields computed per month, and `previous()`

```markdown
fields:
  carry over: yes/no, default no
  assigned: per month = sum(from(assignments.category, month = period).amount)
  spent: per month = sum(from(transactions.category, month(date) = period).amount)
  available: per month = assigned + spent + if(carry over, previous(available), max(previous(available), 0))
```

The exact additions:

- `per month` before `=` makes the field one value **per month** for each
  item, kept in the cache under (item, field, month). `period` is the month
  being computed.
- `previous(field)` is the same field of the same item one month earlier.
  It is allowed only in a `per month` field, and it is 0 or empty before
  the first month. It never reaches another item, so it adds no hop.
- `min` and `max` also accept two numbers (`max(x, 0)`); today they take a
  list.
- The range of months follows Actual: from the first month with data in
  any input to 12 months after today. No setting is needed.
- **Cycle rule:** the same as today, field to field. `previous(available)`
  inside `available` is not a cycle, because it reads an earlier month.

What it costs the shared library (estimates, not measured):

- **Cache:** 50 categories × 132 months (ten years plus one ahead) × 4
  fields is about 26,000 numbers, well under a megabyte on a phone.
- **One transaction changed:** one indexed sum for that category's month,
  then up to 132 additions forward for that category's `available`, then
  the month totals (case 2). That is thousands of additions, so it should
  take well under a millisecond. Actual does the same work in the same
  order.
- **Cold build:** one grouped pass per `from()` ("sum by category and
  month"), as Actual's `getSumAmountsByMonth` does, instead of one query per
  cell.
- **The 1,000 cap:** Groceries over five years has well over 1,000
  transactions, but one month has far fewer. The cap must therefore count
  the items *after* the condition, and the engine must answer
  `month(date) = period` from an index on the link and the date. Reading
  every linked item first would not work (question F4).
- **New engine pieces:** a period-keyed cache, forward recompute along
  months, and the range rule. A month picker on collections over a
  per-month kind is also needed, since a budget screen shows one month.
- **Offline:** works as S3 says. The device computes the same numbers with
  the same library and marks them not yet confirmed.

Not covered: Actual's carryover flag is set per month, while this sketch
sets it per category. A per-month flag would need stored per-month values,
which is a further step. Also not covered: Age of Money, which needs a
first-in, first-out queue. Both Actual and YNAB compute it in report code;
here it would be a coded operation that draws a chart when opened, not a
field.

### (c) What the closest product does

Actual: a cell per category per month whose code reads last month's cell
(§2.1). Design (b) is the same shape written in a kind file instead of
TypeScript. YNAB computes the same thing on its side and publishes
`balance` per category per month.

## 7. Case 2: "Ready to Assign", a total over every category

The need: income this month, plus what was unassigned last month, minus
everything assigned this month, minus last month's overspending in
categories that do not carry it.

### (a) Formulas unchanged; computed by code

A `code` value on a budget-month item or on the plugin. It has the same
costs as case 1 (a), and it is the number the person looks at first.

### (b) A small growth: totals over all items of a kind

```markdown
fields:
  overspent: per month = if(carry over, 0, min(available, 0))
totals:
  ready to assign: per month = previous(ready to assign)
      + sum(all(categories, group = "Income").spent)
      - sum(all(categories, group != "Income").assigned)
      + sum(all(categories).previous(overspent))
```

The exact additions:

- A `totals:` section in a kind file names values over the whole kind. They
  are computed like fields, may be `per month`, and are shown in a
  collection's header or a sidebar line.
- `all(kind[, condition])` is every item of the kind. The 1,000 cap applies
  as for `from()`. Categories, accounts and mailboxes are small kinds;
  `all(transactions)` over years would exceed the cap and show "too many
  items", which steers the author to a per-month field.
- Here `previous()` may follow a dot on a list (`all(...).previous(overspent)`).
  That is the same one-step-back read for each item.
- The sketch assumes income is booked to categories in an Income group, as
  Actual does. The stress test's Money has no such group yet.

Cost: about 50 additions per month per total, recomputed forward from the
changed month. A change to one category's `assigned` dirties that month's
total and every later one. "Buffered" (holding money for next month) would
be one more stored per-month value and is left out of this sketch.

### (c) What the closest product does

Actual: summary cells per month sheet (`from-last-month`, `available-funds`,
`last-month-overspent`, `to-budget`), each a fixed function of named cells
(§2.1). YNAB: `to_be_budgeted` per month in the API, with the same "cash
overspending reduces next month" rule. Notion and Airtable would need one
"Budget" row related to every category, plus a rollup; and because Notion
cannot roll up a rollup, the carry from last month would be impossible.

## 8. Case 3: Mail Inbox, a condition two links away

The need: a thread is in the Inbox if any of its messages is in a mailbox
whose role is Inbox (thread ← messages → mailbox → role).

### (a) Formulas unchanged; computed by code

`in inbox: code` on threads, set by the sync operation, which already
knows when messages move. Offline cost is small here: moving mail needs
the server (IMAP), so an archive made offline waits as "not sent yet"
anyway (S1).

### (b) No growth: one computed field per hop

```markdown
<!-- kinds/messages.md -->
  in inbox: = mailbox.role = "Inbox"

<!-- kinds/threads.md -->
  in inbox: = count(from(messages.thread, in inbox)) > 0

<!-- collections/inbox.md -->
where: in inbox
```

Each field goes one hop, and computed fields may use computed fields
(format §1.6). The engine's cost: a thread reads at most 1,000 messages. A
message move dirties one message and its thread. A mailbox's role changing
dirties every message in it (tens of thousands in an archive), but that is
rare and must run off the request path, in batches. `where: in inbox`
reads the cached value from the index.

A larger growth, allowing `from(messages.thread, mailbox.role = "Inbox")`
inline, would make the engine track the same two levels of dependency
behind the author's back. It saves one line and hides the cost.

### (c) What the closest product does

Airtable: a lookup field per hop, exactly (b). Notion and Bases follow
links to any depth inline (`Parent Task.Sub-item.every(...)`,
`link.asFile().properties`). Notion states no limit; Bases has none
documented.

## 9. Questions for the owner

**F1. Where do numbers that carry from month to month come from?**
*What:* a budget category's "available" includes what was left last month.
That is a value depending on itself one month earlier, which today's
formulas cannot say.
*Why now:* it decides whether Money's main number moves offline, and
whether the formula engine needs a per-month cache before it is built.
*Example:* on a plane you add a €40 grocery transaction. Should Groceries
show €60 instead of €100 at once, or after landing?
*Options:*
(a) **"Computed by code" fields.** The plugin's server code computes and
stores them. Cost: the number stays at €100 until the phone reconnects;
each budgeting plugin writes the tricky recompute rules itself; and the
host needs a cache-writing call.
(b) **Per-month fields with `previous()`.** The kind file says
`available: per month = assigned + spent + previous(available)`. Cost: the
shared library gains a cache keyed by month, forward recompute and a month
range, and collections need a month picker.
(c) **Both.** Cost: two ways to do the same thing, and authors pick the
offline-blind one because it feels familiar.
*Recommendation:* (b). It is what Actual does in code, written as a
declaration. It keeps S3's promise for the number people look at most, and
it stays within the rule that formulas never write.

**F2. May a formula total every item of a kind?**
*What:* Ready to Assign adds up every category, and no category links to
anything that could collect them.
*Why now:* without it, Money's header number needs code or an artificial
"budget" item.
*Example:* `ready to assign: per month = previous(ready to assign) + income − sum(all(categories).assigned)`.
*Options:*
(a) **A `totals:` section with `all(kind[, condition])`**, under the same
1,000 cap. Cost: one more section in kind files; totals over large kinds
show "too many items" instead of a value.
(b) **One "budget" item that every category links to**, as people do in
Notion and Airtable. Cost: a fake item and link in every space, which a
person can delete by mistake.
(c) **Code.** This has the costs of F1 (a).
*Recommendation:* (a). The categories kind is small. The cap turns the
expensive case into a clear message instead of a slow phone.

**F3. Do formulas stay at one hop?**
*What:* the Mail Inbox needs thread → messages → mailbox role. One
intermediate field per hop expresses it today.
*Why now:* the hop rule sets the engine's dependency tracking, which is
much simpler at one level.
*Example:* `in inbox: = mailbox.role = "Inbox"` on messages, then
`in inbox: = count(from(messages.thread, in inbox)) > 0` on threads.
*Options:*
(a) **Keep one hop and document the pattern.** Cost: one extra line for
each further hop.
(b) **Allow two hops inline.** Cost: the engine tracks two levels of
dependency, and the cost is hidden from the author.
(c) **Any depth, as in Notion.** Cost: unbounded work per item, and the
1,000 cap multiplies at every hop.
*Recommendation:* (a). The work is the same as (b), and every cached
value stays visible and named. Airtable works this way.

**F4. Does the 1,000-item cap count before or after the condition?**
*What:* `from(transactions.category, month(date) = period)` over Groceries
reads one month, but Groceries has thousands of transactions over the
years.
*Why now:* if the cap counts before the condition, per-month budgets break
after about two years of use.
*Example:* five years of groceries is about 1,300 transactions, and about
20 in one month.
*Options:*
(a) **After the condition, answered from an index** on the link field and
the date's month. Cost: the engine keeps that index for every link field
whose kind has a date field.
(b) **Before the condition.** Cost: long-lived categories hit the cap, and
budgets stop working for loyal users.
*Recommendation:* (a). It is what Actual's SQL does with its date-range
queries, and the index is needed for `from()` anyway.

## 10. What could not be verified

- Where Notion and YNAB compute (server or device) and how they behave
  offline. Their pages do not say.
- Notion's limits on formula depth, list length and recompute timing. The
  pages read state none.
- Coda's official formula documentation, which has moved (Coda is now
  Superhuman). Only community answers were read.
- Whether Actual's `QUERY` and `BUDGET_QUERY` functions have row limits, and
  how Actual's tracking budget differs in detail. Only cell names in
  `tracking.ts` were skimmed.
- Every cost figure in §§6–8 is a back-of-the-envelope estimate, not a
  measurement. Measuring would need the engine (issue #108's lane) and the
  perf VMs.
- The AQL compiler (`server/aql/`) was not read in depth.

## Simplification opportunities

- **Drop `field: code` from the stress test's invention list.** Every case
  it covered is either a per-month field (F1) or one field per hop (F3).
  Reports that need real code (Age of Money) are operations that draw a
  view, not stored fields.
- **Correct stress-test rows 3 and 8.** Row 8 (two hops) is expressible
  today. Row 3 becomes `totals:` with `all()`.
- **Budget targets are not formulas.** Actual's templates and YNAB's
  targets compute an amount, and a button writes it. In our design that is
  a target field, a computed `needed: per month = …` field, and an
  operations-list action `fill from target`. No template language is
  needed.
- **One recompute order for everything.** Actual's engine is a single
  dirty set sorted topologically. Our formulas, per-month fields and totals
  can share that one mechanism instead of separate invalidation code for
  `from()`, links and months.
- **Cap and index together.** Stating the 1,000 cap as "after the
  condition, from an index" removes the temptation to add special
  date-range functions later.
