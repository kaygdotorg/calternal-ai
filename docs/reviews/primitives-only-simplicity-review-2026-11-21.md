Second independent review, simplicity and authoring (Sonnet 5.5, medium reasoning), commissioned by the coordinator on 2026-11-20 night after the security review, whose findings it was given. Reviewed commit: 6c432d33 on `lane/v2-host-base`. The reviewer states it read the integrated design and `authoring.md` in full and the other sections in part.

# Second review: simplicity and authoring (commit 6c432d33)

I read the integrated design, `authoring.md` in full, and parts of `files-and-questions.md`, `running-code.md` and `permissions-and-calls.md`. I skimmed `drawing.md` and did not open `prior-art.md` or `was-and-is.md`. The numbers below for host tables and checks are my own count from the sections I read, so treat them as estimates. I do not repeat the 21 security findings.

## 1. Is it actually simple?

**(a) Plain app author.** The claim of 15 concepts holds only for Reading List. The 15 are top-level nouns. They leave out what the code actually uses, and the API surface adds up to about as many names as revision 14 had concepts.

- **API surface (`authoring.md` §1 "Counts").** 14 declarations, 40 calls and 6 box calls make 60 names. Revision 14 had "about 60 concepts" for a Chat-class plugin. The vocabulary has moved from the contract into the SDK; it has not shrunk.
- **Chat author's extra nouns.** I count about 45 in total. These are:
  - Record policies: `edit`, `redact`, `author`, `author-or-manager`, `writers`.
  - Record settings: `position`, `segment`, `anchor`, `expected`, `request`.
  - The `where`/`sort`/`linksTo` query strings, with `now`, `null` and `!`.
  - The `me.` namespace: `me.unread`, `me.muted`.
  - Derived look fields: `site(url)`, `last.excerpt`, `records.selection`.
  - Slots: `effect`, `recommended`, `gives`, `may`.
  - Actions: `compose`, `when: "own"`, `place`, `confirm`.
  - Grants: `depends`, `grant.calls`, `public`, `returns`, `phrases`.
  - Run API: `work.start` with 10 parameters, `reads.consent`, `key`, `withheld`, `Stamp.carries`.
- **One thing named differently across sections.**

| Thing | Names used |
| --- | --- |
| Edit/delete a record | `record.edit/redact` (files), `host.records.replace/tombstone` (permissions), "tombstone" (drawing) |
| What is run | work, run, unit, program, harness, box, sandbox, hosted, machine, place |
| What an agent is | "agent", "marked chain", "outbound-capable", `agent: true` |
| A list | collection, list, section, strip, facet, window, source |
| Who may do what | right, grant, capability, consent, ask, pick, designation, handle |
| Read position | `position`, `after_position`, `me.unread`, `state.read.<chat>` |
| Primitive count | `authoring.md` still says "seven primitives" and numbers drawing "6" and the outside "7"; the integrated design says five |

- **Numbers in the integrated design that contradict `authoring.md`.** Notes is 36 lines plus 11 interface lines in the integrated doc §5.2 and 29 lines in `authoring.md` §2.2. Reading List is 11 lines in the integrated doc and 10 in `authoring.md`.

**(b) Host implementer.** "Five primitives" is a way of describing the design. The build is larger.

| Kind | Count (estimated) | Items |
| --- | --- | --- |
| Tables and stores | about 16 | auth.db tables, ledger, shares, approvals, consents, picks, audit, secrets, machines, the work table, and 5 index tables (files, per-kind, records, links, aliases) plus FTS5 |
| State machines | about 9 | work (5 states and 6 end reasons), run log sync (sequence numbers, compare-and-set, epoch), consent ask (ask, 60 s wait, freeze), rename (commit, alias, rewrite), redact with history scrub, install and update approval, pick and fallback, segment rollover, handle MAC |
| Background jobs | about 11 | index publisher, link rewriter, watch fanout per person, alarm scheduler, 24-hour mirror, receipt retention, audit retention, trash retention, history scrubber, quota sweeper, tiering |
| Distinct checks | about 15 | authorize steps 1–6, designation, marked chain, record policy, path containment, egress and private addresses, secret echo, frame clipping, quotas, share challenge, passkey, pick eligibility, mute drop, namespace claim, grant digest |

**(c) Person using the product.** These concepts reach the user:

- **Decisions:** install sentences, picks (per person and per space), "Always" versus "ask each time", the one-minute withhold, and a passkey for the riskiest steps.
- **Screens and states:** the challenge sheet, "Needs you", "runs on Kim's machine", `Settings/Access.md`, and "My sidebars and settings".
- **Visible markers:** the "withheld" markers, "deleted by a manager", and a link that changes meaning after another person's rename.
- **Hidden from users:** segments (if Q20a holds) and handles.

## 2. Complexity that crept back in

| Mechanism | Verdict | Replacement |
| --- | --- | --- |
| Record policies (`author`, `author-or-manager`, `writers`) | Cut the knob | One host rule: author edits or redacts; manager redacts only if Q15 says so. A log kind is a plain file with append and no records. |
| Monthly segments (Q20) | Cut for now | A cap exists only because "edit in place" is believed to cost a rewrite. Measure append and redact on a 64 MiB file first. If the cost is flat, there is no segment concept, no Q20, and no "chat vs this month's file" problem (`authoring.md` §3.6). |
| Designation rule and handles (MAC, 1 hour expiry) | Replace | See "Reach set" below. This also absorbs F10 and F9. |
| Marked chains | Keep, redefine | Apply the rule at the point of delivery: anything delivered to an outbound-capable plugin or its run is filtered by author consent. That is one static check, with no chain to carry. |
| Withheld markers | Simplify | Drop silently and give a single `omitted: n` count. `r.withheld` filtering in every plugin (`authoring.md` notify, harness) is boilerplate an author gets wrong. |
| Facets | Cut | `authoring.md` already says "a collection with `group`". |
| Declarative `create`/`set` (Q22) | Cut | See below. |
| Two files per run | Keep the protocol, drop the metaphor | It is three host calls: append to log with a sequence number, read inbox after a sequence number, end. Describe it that way. |
| Per-person state batching (5 s) | Hide | It is the host's write coalescing and should not be a concept anywhere an author reads. |
| Slot kinds (`effect`, `recommended`, `gives`, `may`) | Shrink | A slot is a named operation signature plus a picker. Derive `gives` and `may` from the signature. Keep `effect` and `recommended`. |
| Challenge, consent, pick, install and Stop screens | Merge | One host sheet with one data shape: who, what, buttons. Then they are one implementation. |
| Host link rewriting (Q18, F14) | Move to a plugin | A rename keeps an alias. Notes offers "fix links", which does ordinary stamped writes with Notes' own rights. Files the renamer cannot write are left alone, and the alias resolves. This deletes the cross-author rewrite and the background job, and F14 goes with it. |
| Third storage class (Q1) | Settle by principle | It is the same class as `auth.db` and history today (ADR 0062). Ask the owner for a one-line confirmation, not a decision. |

**Declarative operations are redundant with grants.** `kinds:{reading:"write"}` already lets the app's own UI call the host's `files.create` and `files.set` directly. An Action can be `{ set: { read: true } }` on a row, and "New" is a form derived from the kind. Reading List then has a kind and a sidebar and nothing else. It also removes an ambiguity: `done: set("reading", { read: true })` never says which item it targets.

**Reach set instead of handles.** The host keeps, per session, the items the person has open. It also keeps, per run, the items named in `work.start`'s input, until Stop. A link argument in a cross-plugin call is valid if the caller could reach it by grant, or it is in that reach set at the time of the call. There is no token, key, expiry or replay window. It fixes F10 (the person has left chat A, so A is no longer open), F9 (run-scoped) and F13 (the host resolves the program from the slot pick). The composer receives the target as `ctx.target`, not as an input.

## 3. Where the primitives overlap

- **Questions is the read half of Files.** A collection is a file, and the index is a view of files. **Running code is the deferred half of Calls:** a work row is a call with `at`, a place and a log, and a receipt is a row that has already ended.
- **Consent is spread over three primitives.** It lives in Authority but is triggered inside `work.start({reads})` (Running) and drawn by Drawing.
- **Looks carry an undeclared expression language.** `me.unread`, `last.excerpt` and `site(url)` belong to Questions and Drawing together. The files section writes `where` as YAML (`{ read: false }`) and the SDK writes it as a string (`"!read"`). That is two query syntaxes.

I would explain the design as three primitives, while the build still follows the five:

```
Files  = items, history, stamps, and ask (index/collection/query/windows)
Calls  = who may do what: grant, consent, and run-later/run-elsewhere (work, box, log)
Drawing
```

- **Reading List:** a kind and two collections under Files and Drawing. There is no Calls code.
- **Chat:** `chats` is a record kind; `send` and `reply` are Calls; the agent is a call that runs elsewhere; the composer is a Drawing slot filling a Calls signature.

This changes the explanation, not the build order.

## 4. Authoring reality check

**Things a day-one author trips on, found in `authoring.md`:**

- **Mute never reaches the notifier.** `state` is "per person and plugin". Chat writes `muted.<chat>` and `calternal-chat-notify` reads `ctx.state.get("muted.…")`, so it reads its own empty state. Q31 says the host drops a push for a muted recipient, but "for that plugin" does not say which plugin's state is meant. This is a real bug, and the one case of "must declare twice".
- **Read position uses two names.** The kind says `position: "read"` and `send` writes `read.${chat}`. The key does not match, so unread never works.
- **A chat message is sent before consent is known** (`chat-ai/call`). The prompt is posted through `calternal-chat/send`, then `work.start` waits up to a minute. A refused or abandoned ask leaves a chat message with no agent behind it.
- **`steer` and `stop` have no actor check.** The error text is "No agent is working in this chat." in one place and nothing in the others. F5's fix needs a message too, and none is written.
- **Recurring work has no form.** Mail polling or a daily summary is "re-arm yourself", and the host "never decides". Every author invents a key scheme (`authoring.md` §3.9). Notes' snooze does not even cancel on unsnooze.
- **Request IDs on every write** (`request: ctx.request`) is noise the author gets wrong. Q5(a) fixes it. I would decide it without asking.
- **Mentions are `@<id>` in text** while the composer shows names (§3.10). This cannot work as written.
- **Official plugin ids are inconsistent.** The owner's rule says `calternal-<name>`, but `reading-list` and `notes` break it.
- **Reading List needs three sections to get right.** The kind is in files, the look in drawing and the grant in permissions.

**Developer loop.**

| Step | Needed | Cost |
| --- | --- | --- |
| Create | `calternal new` template | trivial |
| Run with reload | `calternal dev`: one-space host, dev trust, reinstall on save | cheap if it is the real host binary; "a local machine runner for programs" is not cheap |
| Test | `authoring.md` wants a fake host with "the same `authorize`" | Do not build it. A second implementation will drift, since Wasm is ruled out and the fake would be TypeScript. Run tests against the real host in a `--test` mode: temp directory, fake clock, scripted people. |
| Validate | the installer's validator as a command; it prints the install sentences | cheap, and it must be the same binary |
| Publish | signing and a gallery | defer. Q6(a) says an operator-pinned list, so `publish` is a pull request for now. |

I would add one thing the loop lacks: `calternal check` should print the derived `outbound` word and the sentence the owner will see, so an author learns the cost of a grant before shipping.

## 5. Power check

| What a serious author wants | Inside the model? | Cost |
| --- | --- | --- |
| Commands, keyboard shortcuts and a palette (Obsidian, VS Code, Raycast) | Yes. An operation with a title can be a command. | A `shortcut` field on an operation; the shell renders it. |
| Plugin settings (all four) | Partly. There is a `form` part and per-person state, but no stated "settings page" shape. | One page type bound to state keys. |
| A plugin's own rebuildable cache (Mail's index, an importer's map) | No. Userdata is strictly files, appdata is the host's, and a plugin gets neither. | A per-plugin rebuildable store that the host may delete; needs the owner's decision (missing question 1). |
| Markdown renderers (Obsidian code blocks, mermaid) and editor extensions | Only through a frame, which Q9 says leaks. | Declarative editor commands (slash-insert, autocomplete lists), the same shape as the composer's `insert`. |
| Inbound webhooks and external events | No. | A host route that maps a signed request to an operation with a fixed actor; needs rate and signature rules. |
| IMAP and SMTP (Mail) | No (security F16). | A host-held connection method. This is a host feature and not a plugin one. |
| Bulk import of 3,000 notes | Yes, but the quotas will block it. | An operator-raisable "import" quota class. |
| Cross-space views | Unknown. The design never says. | A question for the owner. |
| Theme or appearance changes (VS Code, Obsidian) | No, and rightly so. | The frost dial already exists in the Mac app. |

## 6. The build plan

Step 1 is not the smallest running thing. "Install through the one validator" and "get the row from the query" pull in the validator and most of the index, which the plan puts in step 2.

**The smallest running thing would be:** one hardcoded kind, one file written through the host with a stamp in the ledger, a directory listing as the query, and no validator. That is a one-day spike, and every call signature in it already carries the actor and request. If the actor is missing from the first call signature, adding Authority in step 3 is a retrofit.

**The riskiest assumption is Drawing.** It is step 9, last, a one to two week estimate "for interface lanes". Every claim about kinds, looks, a records view and "plugins never draw" depends on whether a closed catalogue can draw Chat's timeline and Notes' editor on SwiftUI. Start a drawing spike alongside step 1: Reading List and a Chat transcript on the iPhone Simulator from declarations only. It will change kinds and looks if it is going to.

**Unmeasured costs to prototype first:**

- the live-window save path against the 40 ms gate, without stored row sets;
- append and redact on a large record file, with history scrubbing, which decides whether segments exist;
- plugin process memory and cold start, from #103;
- the warm-path check cost ("0 disk reads");
- the change-chain depth of 4 and the 50 ms batching;
- the kill-the-server run test (step 6);
- whether V8 isolates are a boundary. The text itself says "probably not".

## 7. The 34 owner questions

**Decide by the design's own principles; ask the owner to confirm.**

| Q | Decision and reason |
| --- | --- |
| 1 | Host state is the same class as `auth.db` (ADR 0062). One line. |
| 4 | (a). Owner rule: first-party is not special. |
| 5 | Decide only after #103. Prefer one process per (plugin, person), see "One mechanism" below. |
| 10, 30 | (a). The owner chose native system controls and closed drawing. |
| 12 | Neither as written; use the security review's fenced `log.call`. |
| 17 | (a). |
| 18 | (b), or my "fix links" plugin. This follows F14. |
| 19 | (a). |
| 20 | Cut, if the measurement above is flat. |
| 21 | (a). It is the owner's "dumb" principle. |
| 22, 23 | Moot once declarative operations are cut, and the slot follows the pick. |
| 27, 28, 29 | Operator defaults. Do not ask. |
| 31 | The host drops a push for a muted recipient. It is the owner's per-person mute. |
| 32 | (b). Convert on import. One link form (ADR 0024). |
| 33 | (b). |
| 34 | Already decided by "split by effect". |
| 24, 25, 26 | Design questions for the mockup. They do not block the build. |
| 13 | (a). |
| 14 | Defer. Collection fields are a feature, and they are not needed for step 9. |
| 8 | (b), plus the agent rule. Per-person approval of every plugin adds a prompt without a fixed risk. |

**Real forks the owner must answer.**

- **Q3** (what counts as an agent and which files are protected).
- **Q6** (who approves installs, updates and sources).
- **Q7** (whose machine runs an agent in a shared space).
- **Q9** (frames in browsers).
- **Q11** (how far delete reaches).
- **Q15** (manager moderation).
- **Q16** (passkey).
- **Q2** (strict versus delegated), now cheaper to explain: the reach set above is delegated with no tokens.

**Missing questions (mine, in addition to the security review's 11).**

1. Does a plugin get its own rebuildable store?
2. What does a person install, an app or a plugin? Chat is four plugins and four install screens.
3. Do collections and sidebars span spaces?
4. May a plugin be driven by an incoming webhook, and as whom?

**Twenty-minute order.**

| Block | Time | Questions |
| --- | --- | --- |
| Confirm in a table, one line each | 5 min | the "decide by principle" rows |
| Trust (Q3, Q6, Q7, Q16, Q9) | 8 min | |
| Data (Q11, Q15, Q2) | 4 min | |
| Missing questions 1 to 4 | 3 min | |

## 8. Verdict and five changes

The design is sound in its core idea of one table, one check, and host-held stamps and consent. It is not yet as simple as the owner asked. The author-facing surface has 60 names, the host has about 16 stores and 11 jobs, and several mechanisms are app ideas in disguise (segments, facets, declarative ops, link rewriting, handles). The security review's fixes are mostly free for authors, but two add burden. First, taint per process would smear a normal Chat process: it would stamp Sam's message `carries Lee` after serving Lee. Second, F6's per-record diff of every byte change is heavy for the host.

**One mechanism for G1, F15 and G2.** One worker process per (plugin, acting person), stopped when idle. Taint then equals the authors that process has seen, and it covers only outbound-capable plugins. This closes cross-person recall and removes the need for per-call or per-process taint rules. Its memory cost needs the #103 numbers.

**A cheaper F6.** A byte write to a record file that is not a pure append of host-formatted records is a conflict copy, or an `outside`-stamped append that agents do not see. Checking "the old bytes are a prefix" is cheap and needs no per-record diff.

**Five most valuable changes, ranked:**

1. **Decide the drawing question first.** Run the Drawing spike next to step 1, and have a one-day Files-and-ledger spike as step 1, with the actor in every signature.
2. **Cut declarative operations, facets, record-policy knobs and the host link rewriter.** Reading List becomes a kind and a sidebar; Chat's policy becomes one rule.
3. **Replace handles and the designation rule with the reach set.** It fixes F9, F10 and F13 in one host-held set, and the author sees nothing.
4. **Settle the names and fix the plugin bugs in `authoring.md`** (mute state, `position` key, consent-before-send, mention tokens, recurring work, the `calternal-` prefix), then freeze the SDK vocabulary before any code.
5. **Test against the real host in a `--test` mode** and not a fake. Measure window cost, large record files and process memory before committing to segments or the one-process-per-person rule.
