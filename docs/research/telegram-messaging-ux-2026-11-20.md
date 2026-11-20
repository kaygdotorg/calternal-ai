# Telegram messaging UX: what Threads should adopt

Date: 2026-11-20. Lane: `lane/telegram-study`. Research only; no product code.

Scope: the owner called Telegram "the god-tier of messaging app UX" and
`docs/morning-2026-11-20.md` (on `dev`) says Telegram is the reference for
Threads messaging ("edit and delete, unread counts, mentions and push
first"). Threads carries conversations with people and agents as
participants of one kind of conversation. This note records how Telegram
behaves, from public sources only, and what Threads should take.

Method: Telegram's public docs, FAQ, blog and API schema pages were read.
Firecrawl refuses `telegram.org` ("we do not support this site"), so those
pages were fetched with plain `curl` and stripped to text; third-party pages
went through Firecrawl. No Telegram app or account was opened and nothing was
captured from a real conversation.

## How to read the tables

- **Verified** items cite a URL. **Unverified** means no public source read
  states it; those rows say so and rest on general knowledge of the app or on
  design inference. Treat them as hypotheses to check by building.
- Decision column: **adopt** (copy), **adapt** (copy with a stated change),
  **skip**.
- Cost: S ≈ a day or less, M ≈ days, L ≈ a week or more, for one engineer on
  the Rust host plus SwiftUI and Svelte clients. These are rough guesses, not
  estimates from a plan.

## Threads constraints that change the answers

Taken from `docs/morning-2026-11-20.md`; they decide several rows below.

- A conversation is a **Markdown file** with a deep link to each message,
  owned by Threads. Telegram keeps messages in a server database with
  per-chat integer IDs; Threads has files. Features that Telegram does with
  server counters (unread counts, mention counts, read pointers, drafts)
  need a per-person state file or table beside the shared conversation file.
  Whether the file is shared or a copy per person is still an open question
  in the morning note.
- No server-wide people discovery: people are found only inside a group.
  Telegram's global username search is therefore a **skip**.
- Protected like notes now, end-to-end encryption later. Server-side
  search, previews in push, and bot reading all depend on that; see the bots
  and notifications areas.
- Agents are participants but cannot read conversations between people
  unless invited, and consent is needed from each person whose messages are
  included.

## 1. Chat list

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| Ordered by last activity, newest on top. Pinned chats stay above; default limit 5 pinned (10 Premium); each folder has its own pin list (limit 100/200). Source: [config](https://core.telegram.org/api/config), [FAQ](https://telegram.org/faq). | Adopt order and pins. Adapt limits: no tiers, pick one pin limit. | Ordering by activity plus a few pins is what makes a long list navigable. Limit values are Telegram's commercial tiering. | S |
| Per-chat unread count (`unread_count`) and a separate `unread_mentions_count`; a blue mention indicator sits next to the counter ([mentions](https://core.telegram.org/api/mentions)). Muted chats show a grey badge (grey vs blue is visible in the bug reports at [bugs.telegram.org/c/12370](https://bugs.telegram.org/c/12370); exact colours otherwise **unverified**). | Adopt. | Counts that distinguish "something" from "something for you" are the core triage signal. Needs the per-person read state noted above. | M |
| Read state is a pointer: `read_inbox_max_id` and `read_outbox_max_id` per dialog ([mentions](https://core.telegram.org/api/mentions) dialog schema). A manual "mark as unread" flag exists (`unread_mark`). | Adopt the pointer model, and the manual flag. | One id per person per conversation is cheap to store and sync; count is derived. Message ids must therefore be ordered within a thread. | S |
| Folders shown as tabs. A folder is a filter: include/exclude by type (contacts, groups, bots), muted, read, archived, plus explicit chat lists; up to 10 folders (30 Premium), 100/200 chats each ([folders](https://core.telegram.org/api/folders), [blog](https://telegram.org/blog/folders)). Folders are shared as a link ([blog](https://telegram.org/blog/folders)). Folder tags (coloured labels under the preview) exist but are Business-only ([folders](https://core.telegram.org/api/folders)). | Adapt: folders as saved filters, with "agents" and "people" as built-in kinds. Skip shared folders and tags for now. | Folders are the main answer to "too many chats". Threads will mix agent runs (many, noisy) and people (few, important), so a kind filter earns its place early. Sharing folders crosses group boundaries and conflicts with no discovery. | M |
| Archive: swipe left to archive. An archived chat that gets a notification pops back to the main list; a muted archived chat stays archived forever. Archive can be hidden by swiping left on it and revealed by pulling down ([blog](https://telegram.org/blog/archive-and-new-design)). The archive's unread badge counts muted chats too, which users ask to change ([tdesktop #25829](https://github.com/telegramdesktop/tdesktop/issues/25829)). | Adopt the pop-out rule; adapt the badge: exclude muted. | Pop-out-on-activity is what makes archive safe to use. The reported badge complaint is a known trap to avoid. | S |
| Drafts: unfinished text is shown in the chat list in place of the last message, the chat moves to the top, and the draft syncs to all devices ([blog](https://telegram.org/blog/drafts), [API](https://core.telegram.org/api/drafts)). Cleared on send. The server pushes `updateDraftMessage` to other devices. | Adopt (list preview and sync). Adapt: store the draft in the per-person state file, not the shared thread file. | Drafts must never appear in the shared file. Moving the chat to the top on draft is Telegram's choice; Threads could skip the reorder. The reorder is **unverified** as still current. | S–M |
| Swipe actions: swipe left on a row archives ([blog](https://telegram.org/blog/archive-and-new-design)); long-press opens bulk select with pin, mute, archive, delete ([blog](https://telegram.org/blog/archive-and-new-design)). Other swipe actions (read/unread, pin, mute) are **unverified** from a source. | Adopt swipe-to-archive and multi-select; use system `swipeActions` and list selection on Apple platforms (AGENTS.md rule). | System controls already give these; custom gesture code is not justified. | S |
| Search field at the top of the list searches chats, messages and usernames ([FAQ](https://telegram.org/faq)). | Adapt: chats and messages; people by group only. | See Search. | See Search |

## 2. Conversation view

Most of this area is behaviour of the apps, not of the protocol, and the public docs say little. Rows marked **unverified** rest on general knowledge of the app.

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| Consecutive messages from one sender are grouped: one avatar and name for a run, tight spacing inside the run. **Unverified** from a source. | Adopt. | Reduces repeated chrome; standard in every chat UI. | S |
| Date separators inside the history, and the date bar in Shared Media opens a calendar to jump by date ([blog](https://telegram.org/blog/shared-media-scrolling-calendar-join-requests-and-more)). In one-to-one chats, tapping the date bar offers deleting a day or range ([FAQ](https://telegram.org/faq)). The in-history separators themselves are **unverified** from a source. | Adopt separators. Adapt: a jump-to-date control later; skip day deletion. | Separators are cheap. Delete-by-day conflicts with the message-file model. | S |
| A chat opens at the first unread message with an "unread messages" separator line; the tdesktop tracker has bugs where the chat jumps to the bottom and marks everything read ([tdesktop #27189](https://github.com/telegramdesktop/tdesktop/issues/27189)). The opening position and separator are **unverified** from an official source. | Adopt: open at the first unread, draw the separator, and mark read only as messages actually scroll into view. | The reported bug is exactly the failure to avoid: a jump to the bottom that silently clears unread. | M |
| A floating "jump to latest" button with an unread counter; a separate `@` button counts unread mentions and cycles back through them, long-press clears all ([mentions](https://core.telegram.org/api/mentions)). The down-arrow counter is **unverified**. | Adopt both buttons. | The mention button is documented and is the best compact answer to "did anyone need me here". | S |
| Scroll position per chat is remembered. **Unverified.** Reports of scroll "jumping" exist ([bugs.telegram.org/c/9153](https://bugs.telegram.org/c/9153/3), [c/23281](https://bugs.telegram.org/c/23281/8)): history is loaded in slices around an offset id ([offsets](https://core.telegram.org/api/offsets)). | Adopt remembering by message id plus pixel offset. Anchor on a message id, never on a pixel from the top. | Anchoring on ids survives new messages and edits above. The bugs show what happens when slices load without anchoring. | M |
| Tapping a reply's quote scrolls to the original and focuses the exact quoted text "with a beautiful animation" ([blog](https://telegram.org/blog/reply-revolution), [replies](https://telegram.org/blog/replies-mentions-hashtags)). A "back" after the jump is **unverified**. | Adopt jump with highlight; add a "back to where I was" chip (a jump stack). | A jump that cannot be undone makes people lose their place. The chip is our addition. | M |
| Replies also form threads: every reply to message N joins thread N, and replies to replies stay in thread N ([threads](https://core.telegram.org/api/threads)). | Skip for first release. | One flat list plus reply-to is enough. Agent runs already have their own log. Revisit if conversations get long. | L |

## 3. Composing

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| Reply, with the replied message shown above the new one. Quote part of a message: select text, choose Quote; the reply stores `quote_text` and `quote_offset`; reply to another chat is allowed; tapping the quote scrolls to the exact spot ([blog](https://telegram.org/blog/reply-revolution), [threads](https://core.telegram.org/api/threads)). | Adopt reply. Adapt quote: store the message id, quote text and offset. Skip reply-in-another-chat. | Quoting is the natural way to hand an agent "this part". It also yields the exact span an agent may read for consent (see Bots). Cross-chat replies leak context across conversations. | M |
| Edit: hold the message and choose Edit; an "edited" label shows the time of the last edit; admins can edit channel posts ([FAQ](https://telegram.org/faq)). The 48-hour edit limit appears only in user posts and Q&A pages ([Reddit](https://www.reddit.com/r/matrixdotorg/comments/1abd6bf/is_there_a_time_limit_for_the_edit_message/), [Stack Overflow](https://stackoverflow.com/questions/55991350/telegram-bot-cant-delete-messages-that-was-sent-48h-ago-despite-of-it-has-permi)); the FAQ states no limit, and the earlier 48-hour limit on "unsend" (delete for both sides) was lifted ([blog](https://telegram.org/blog/unsend-privacy-emoji)). **Edit limit: unverified.** | Adopt with "edited" and a time. Decide the limit (owner question). Keep prior text only if the owner wants it (Telegram shows none). | An edited message that an agent already read is the awkward case: the agent's context no longer matches. Needs a rule. | S–M |
| Delete: any message you sent or received can be deleted for both sides in a one-to-one chat, with no time limit; in groups you delete only your own; "deleted messages do not leave a mark" ([FAQ](https://telegram.org/faq), [blog](https://telegram.org/blog/unsend-privacy-emoji)). API has a `revoke` flag for everyone vs me ([deleteMessages](https://core.telegram.org/method/messages.deleteMessages)). | Adapt: delete for me (per-person hide) and delete for everyone (sender only). Leave a small tombstone in the shared file. | Telegram's no-trace delete is incompatible with a Markdown file that other people, agents and backups may have read, and with message deep links that must not dangle silently. Needs the owner's call. | M |
| Forward with or without attribution: the API has `drop_author` and `drop_media_captions` ([forwardMessages](https://core.telegram.org/method/messages.forwardMessages)); a "who may forward me" privacy setting makes forwards show a plain, unlinked name ([blog](https://telegram.org/blog/unsend-privacy-emoji)); `noforwards` blocks forwarding ([same schema](https://core.telegram.org/method/messages.forwardMessages)). | Adapt: forward always keeps attribution and a link to the source; skip "hide author". Honour a per-conversation "no forwarding" flag. | Hiding the author would let a person's words be moved into an agent's context without their name, which cuts against the consent rule. | M |
| Mentions: `@username` entity, or a name entity by user id for people without username; typing `@` opens a picker of members of that conversation (a participants filter with a query) ([mentions](https://core.telegram.org/api/mentions)). A mentioned person is notified even when the chat is muted ([blog](https://telegram.org/blog/replies-mentions-hashtags)). | Adopt. Picker lists participants of the conversation only (people and agents), never the wider group. Mentions bypass mute. | Matches the no-discovery rule. The mute bypass is what makes mention-only mode work. | M |
| Drafts | See chat list. | | |
| Scheduled messages: hold Send, choose a time; per-chat schedule queue visible only to the sender; can be silent; can repeat daily to yearly (Premium); a message to your own Saved Messages becomes a reminder ([API](https://core.telegram.org/api/scheduled-messages), [FAQ](https://telegram.org/faq)). "Send when online" exists for one-to-one chats and fails when the recipient hides last seen ([API](https://core.telegram.org/api/scheduled-messages)). | Skip for first release; adapt later (schedule, no repeat, no "when online"). | Needs a server-side timer per message. "Send when online" reveals presence and is a skip. | M |
| Silent send: "Send Without Sound" suppresses the notification sound ([FAQ](https://telegram.org/faq)). | Adopt (soon). | One flag, small. Useful with people on a different time zone. | S |
| Formatting: entities on text (bold, italic, underline, strike, mono, link, quote); desktop shortcuts ⌘B, ⌘I, ⌘U, ⌘K, ⇧⌘M, ⇧⌘X, ⇧⌘N for plain ([entities](https://core.telegram.org/api/entities), [shortcut list](https://www.pie-menu.com/shortcuts/telegram)). | Adapt: messages are Markdown in the file; the composer offers the same shortcuts and writes Markdown. Skip spoilers and custom emoji. | The file format is Markdown, so entity offsets are not needed. | S |

## 4. Message links

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| Per-message link: `t.me/<username>/<id>` for public chats and `t.me/c/<channel>/<id>` for private ones, optional `thread`, `single` (one item of an album), media timestamp, poll option ([links](https://core.telegram.org/api/links)). Message ids are per chat. | Adopt. One link form carrying thread id, message id and optionally a quoted span. | The morning note already requires a deep link per message. A span parameter makes "link to this sentence" free. | S |
| What a link shows to someone without access: for private groups the link only opens for members; others see an "invite to join" screen at best. The exact screen is **unverified** from an official source; the API says only that the private form needs the channel id ([links](https://core.telegram.org/api/links)). | Adapt: show a plain "you are not in this conversation" and nothing from the message, not even the thread title. | Telegram can offer join-by-invite; Threads has no discovery, and a title can leak. | S |
| A reply sent to another chat opens the original only if you can access it ([blog](https://telegram.org/blog/reply-revolution)). | Adopt the same rule for quotes and forwards. | Access is checked at open time. | S |

## 5. Search

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| In-chat search with filters by sender, media, links, voice, mentions-of-me and pinned, plus date; results report an offset and total so the UI can show "3 of 20" ([search](https://core.telegram.org/api/search)). Shortcut ⌘F ([list](https://www.pie-menu.com/shortcuts/telegram)). | Adopt text, sender, date, mentions of me, pinned, links. Skip media types for now. | These cover the questions people ask. The "n of total" counter is cheap and useful. | M |
| Global search across all chats with a folder filter, chats-only / groups-only / users-only flags ([search](https://core.telegram.org/api/search)). Public usernames found in "global results" ([FAQ](https://telegram.org/faq)). | Adapt: search across your own conversations; skip global username search. | No server-wide discovery. | M |
| Server-side search over cloud history ("find the message among millions", [FAQ](https://telegram.org/faq)). | Adapt: index locally per person. | With files and encryption later, a server index conflicts with end-to-end. A local index also keeps agent-private text out of any shared index. | M–L |

## 6. Reactions, read state, typing

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| Reactions: hold a message, pick an emoji; one per person by default (3 Premium); shown under the message with counts; the sender is notified (configurable); hold the counter for who reacted; chats can limit allowed reactions ([FAQ](https://telegram.org/faq), [reactions](https://core.telegram.org/api/reactions)). Short-poll every 15–30 s for visible messages ([reactions](https://core.telegram.org/api/reactions)). | Adopt, soon: small fixed set, one per person. | Reactions are the cheapest acknowledgement and cut noise messages. A reaction on an agent's message could be a feedback signal (our idea). Polling is unnecessary if we push. | M |
| Ticks: one tick delivered to the cloud, two ticks read; in one-to-one and small groups tap for the exact read time, kept 7 days; hiding last seen hides read time too. No "delivered to device" because there are many devices ([FAQ](https://telegram.org/faq)). The read-receipt-off toggle is Premium and **only confirmed by third-party pages** ([Facebook answers](https://www.facebook.com/fb-answers/turn-off-read-receipts-telegram/)); `read_dates_private` appears in the user schema ([pin](https://core.telegram.org/api/pin) schema). | Adopt ticks and the "no per-device status" rule. Adapt: read receipts per person, default on within groups, one switch to turn them off for everyone; keep the 7-day retention for read times. | The 7-day bound meets the review gate "everything stored has a bound". | S–M |
| Last seen / online: three audiences (everybody, contacts, nobody) plus approximate values "recently / within a week / within a month / long ago"; hiding yours hides theirs (except Premium). Typing makes you appear online for about 30 s regardless ([FAQ](https://telegram.org/faq), [blog](https://telegram.org/blog/privacy-revolution)). | Adopt approximate values; skip exact last seen entirely. | Fits groups only; the approximate scale stops stalking while still showing reachability. | S |
| Typing indicator: broadcast as a short-lived "typing…" action; a typing draft TTL of 30 s appears in the client config (`message_typing_draft_ttl`) ([config](https://core.telegram.org/api/config)). A user setting to hide typing is **unverified**; the FAQ says typing makes you visible. | Adopt with a TTL and a privacy switch (our addition). Add an "agent is working" state in the same place. | Not stored, so no file impact. Showing agents as typing/working is the main new use. | S |

## 7. Notifications

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| Per-chat mute: preset or custom duration (8 hours, 2 days, any span); a separate "disable sound" that still notifies ([blog](https://telegram.org/blog/notifications-bots)). | Adopt. | Two levels (silent vs off) cover most needs. | S |
| Mute but still hear mentions and replies: "Mute the group to get notifications only when people mention you or reply to your messages" ([FAQ](https://telegram.org/faq)); replies notify the replied-to person even in a muted chat, per their settings for the sender ([blog](https://telegram.org/blog/replies-mentions-hashtags)). Complaint that replies ignore mute exists ([bugs.telegram.org/c/22902](https://bugs.telegram.org/c/22902/1)). | Adopt as the default for conversations with several people; make it a visible setting. | This is the best idea in Telegram's notification model. Make the bypass explicit so it never surprises. | S |
| Pinned messages notify all members even when the group is muted ([FAQ](https://telegram.org/faq)). | Skip. | A pin should not bypass mute without an admin rule. | S |
| Previews and sounds per chat type, exceptions per chat ([links](https://core.telegram.org/api/links) lists the settings routes). Whether preview text is sent through push is **unverified**. | Adapt: Preview setting per conversation. While not end-to-end, previews are allowed; once end-to-end arrives, show "New message" only. | Push providers see preview text. | M |
| App badge counts unread chats or messages; option to count or ignore muted chats ([bugs.telegram.org/c/203](https://bugs.telegram.org/c/203)). | Adopt with muted excluded by default. | See archive. | S |

## 8. Bots in groups (agents in conversations)

Telegram's model is the closest public precedent for "a person calls an agent in".

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| **Privacy mode** is on by default: a bot in a group sees only commands aimed at it (`/cmd@bot`), general commands if it was the last bot to post, inline messages sent through it, and replies to its messages; it always sees service messages and all private-chat messages. A bot added as admin sees everything; the owner can switch privacy mode off, after which the bot must be re-added. Users can see a bot's privacy setting in the member list ([bots features](https://core.telegram.org/bots/features), [FAQ](https://core.telegram.org/bots/faq)). | Adopt the default and the visible setting. Adapt: the grant is *per message range chosen by the caller*, not per bot. | This matches the morning rules: no access by default, scope chosen at invite time. Telegram's grant is permanent and bot-wide; Threads' grant is scoped and needs consent from every author. | L |
| Adding a bot: from the bot's profile, "add to group", then set rights; or add as a member ([blog](https://telegram.org/blog/notifications-bots)). What every member is told is **unverified**: bots receive "service messages"; a visible join message for the bot is the app's standard but no source read states it. | Adopt: a visible system line "X called agent Y in, reading <scope>" in the conversation, for everyone. | The consent rule needs the people whose messages are read to know when it happens. | M |
| Mentions and replies as the wake-up signal. Newer: "Guest AI bots" act in any chat when called, and ephemeral replies that only the caller sees ([bots features](https://core.telegram.org/bots/features); [blog index](https://telegram.org/blog/reply-revolution)). | Adopt the ephemeral idea: an agent may answer privately to the caller before posting. | Lets someone draft with an agent without exposing the exchange. Needs care about whose subscription pays; see the morning note. | L |
| Bots receive other bots' messages only in an explicit bot-to-bot mode ([bots features](https://core.telegram.org/bots/features)). | Adopt: agents do not read agents by default. | Matches "agents answering agents" still being an open item in the morning note; avoid loops. | S |
| Enforcement: the bot's token, not the group, gates reading, and bots must validate commands themselves ([bots features](https://core.telegram.org/bots/features)). | Skip. | Threads enforces scope in the host, not in the agent. | — |

## 9. Keyboard and desktop (Mac)

Source for shortcuts is a third-party list ([pie-menu](https://www.pie-menu.com/shortcuts/telegram), [usethekeyboard](https://usethekeyboard.com/telegram/)), which may not match the current app. Telegram's own docs list none that were read. Treat each as **unverified** against the current build.

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| ⌘↑ / ⌘↓ choose a message to reply to; ↑ in an empty composer edits your last message; ⌫ deletes the selected message; ⌘F searches the chat; ⌘⇥ / ⇧⌘⇥ next and previous chat; ⌘1–5 jump to folders; ⌥↵ jump to bottom; ⌘0 Saved Messages; Esc cancels reply, edit, search. | Adopt the set where the system has a standard, using SwiftUI/AppKit commands and menus; use the system find bar for ⌘F. | Fast keyboard paths are a large part of the "god-tier" feel on Mac. | S–M |
| Minimize to tray, lock, quit shortcuts; window and notification behaviour not documented here. | Skip tray; use system window behaviour. | Native rules apply. | — |
| Notifications clicked open the chat at the message; inline reply from a notification. **Unverified.** | Adopt (system notification actions). | Cheap on Apple platforms. | S |

## 10. Motion

The public docs do not describe animations beyond two lines: reaction "lottie animations" ([reactions](https://core.telegram.org/api/reactions)) and the quote jump "beautiful animation" ([blog](https://telegram.org/blog/reply-revolution)). Everything below about send, edit, delete and new-message motion is **unverified** from a source and cannot be verified from public documents; it needs a device in hand. Do not treat it as a specification.

| Telegram behaviour | Decision | Reason | Cost |
|---|---|---|---|
| Send, edit, delete and arrival animations (bubble moving from composer into place, collapse on delete, etc.). **Unverified.** | Design our own from the motion tokens (AGENTS.md: order, duration, curve, interruptible, checked on a real device). | The repository rule already requires designed motion. Use system list insert/delete transitions first. | M |
| Reactions and the quote highlight animate; archive pop-out ([blog](https://telegram.org/blog/archive-and-new-design)). | Adopt the quote highlight and the pop-out; defer reaction art. | The two documented animations give meaning (where you landed, what moved). | S |

## Ranked list

### First release
1. One conversation list: ordered by activity, pins, archive with pop-out, unread count per person (read pointer per thread), mention marker, mute (with mention-only), drafts shown in the list and synced.
2. In the conversation: grouping, date separators, open at first unread with a separator, jump-to-latest with a counter, mention button, scroll anchor by message id, jump to a replied-to message with highlight and a "back" chip.
3. Compose: reply with quoted span, edit with "edited" mark, delete for me and for everyone (with a tombstone), mentions with a participant-only picker, Markdown shortcuts.
4. Message deep links with a no-leak page for non-members.
5. Push for mentions, replies and new messages in unmuted chats; badge excludes muted.
6. Agent calls: visible system line; scoped reading chosen at invite time; consent per author; agents see nothing else.
7. Mac keyboard essentials: reply, edit last, search, jump, next chat.

### Soon
- Folders (with built-in people and agent kinds), search inside a chat and across your own chats with a local index.
- Reactions; read ticks with read-time (7-day bound); silent send; approximate last-seen; typing and an "agent is working" state.
- Ephemeral agent replies to the caller; designed motion tokens.

### Later
- Scheduled messages, forwarding, reply threads, shared folders, jump-by-date, media filters.
- Read-receipt and typing privacy switches beyond the first on/off.
- End-to-end encryption interplay: previews, search, agent reading.

## Questions the owner must answer

Each is phrased to stand alone.

1. **Delete for everyone.** Telegram removes the message without any mark. Threads stores conversations as Markdown files that others, agents and backups may already have read. *Decision needed now* because the file layout and link behaviour depend on it. Example: Alice deletes a message Bob already quoted; does Bob's quote still show the text? Options: (a) a tombstone, content removed from the file, quotes lose their text; (b) a tombstone, quotes keep the text (Alice is not truly unsent); (c) Telegram-style no trace and dangling links. Recommendation: (a), because it keeps links honest and takes the text away from the shared file; say so on the delete button.
2. **Edit limit and history.** Telegram's public pages give no confirmed limit and keep no visible history. Example: Alice edits a message an agent already summarised. Options: no limit; a limit (say 48 hours); keep the previous text visible to participants. Recommendation: no limit, "edited" mark with time, previous text kept only in the file's own history for the author and any agent that read it, so a summary can be marked stale.
3. **Where the read pointer, draft and mute live.** The morning note leaves "one shared file or a copy per person" open. Unread counts, drafts and mute are per person. Options: store in a per-person state file beside the shared thread file; or inside each person's copy. Recommendation: per-person state file; never write them into the shared thread.
4. **Mute bypass.** Telegram lets a mention or reply ring through a muted chat. Example: you mute a busy team conversation; someone mentions you. Options: always ring; ring only in mention-only mode; never. Recommendation: default to ring for direct mentions and replies, visible and switchable per conversation.
5. **Read receipts and typing.** Telegram shows read times for 7 days and hides them when you hide last seen. Options: always shown within the conversation; one switch to turn off for everyone; per person. Recommendation: one switch, default on, 7-day bound for the read time.
6. **Agent scope and who is told.** Telegram bots in privacy mode see only commands and replies, permanently. Threads asks the caller to pick one message, a range or everything, with consent from other authors. Example: Alice calls an agent on messages 10–20 written by Alice and Bob; Bob must agree before the agent reads Bob's. Questions: is consent a one-time tap per author or a standing rule? Is Bob told by a visible line (recommended) or a notification only? What happens to messages that arrive after the range: unseen by default?
7. **Whether agents may ever answer privately to the caller** (ephemeral replies, Telegram's newer idea). Costs: the answer is not in the shared file; whose subscription and log it uses. Recommendation: yes later; not in the first release.
8. **Reply threads.** Telegram groups replies into threads. Do we want one flat conversation with reply-to (recommended for the first release) or sub-threads like Telegram's?
9. **Push content.** Until end-to-end encryption arrives, is message text allowed in push previews? Recommendation: yes with a per-conversation switch, and "New message" only once end-to-end is in.
10. **Mac shortcut set.** The list above comes from third-party pages for an older Telegram Desktop; the owner may want to confirm which Telegram shortcuts he uses before we copy them.

## What could not be verified

- Edit time limit (only forum and Q&A sources, no official page).
- The unread separator, the opening position of a chat, scroll position memory and the jump-to-latest counter: not in public docs; only bug reports describe adjacent behaviour.
- Message grouping and date separators inside the history.
- Exact text shown to someone without access to a message link.
- Whether members are told when a bot is added, beyond "bots receive service messages".
- Any motion detail for send, edit, delete and new messages.
- Whether typing can be hidden by a setting; whether read-receipt hiding is Premium-only is from third-party pages.
- Current Mac Telegram Desktop shortcuts (third-party lists).

## Simplification opportunities

- Read pointer plus a derived count removes any need for stored per-message "seen" flags.
- Drafts, mute, pins, folder membership and read pointers are all per-person conversation state: one small file format instead of five.
- Reusing the same "who may this agent read" record for consent, for the visible system line, and for the search index exclusion avoids three separate rules.
- Skipping Telegram's tiers (Premium doubling of limits) and the channel, forum and sticker systems removes most of its surface.

## Sources

- https://telegram.org/faq
- https://telegram.org/blog/archive-and-new-design
- https://telegram.org/blog/folders
- https://telegram.org/blog/drafts
- https://telegram.org/blog/replies-mentions-hashtags
- https://telegram.org/blog/reply-revolution
- https://telegram.org/blog/notifications-bots
- https://telegram.org/blog/unsend-privacy-emoji
- https://telegram.org/blog/privacy-revolution
- https://telegram.org/blog/shared-media-scrolling-calendar-join-requests-and-more
- https://core.telegram.org/api/links, /api/folders, /api/drafts, /api/mentions, /api/threads, /api/pin, /api/reactions, /api/search, /api/scheduled-messages, /api/offsets, /api/config, /api/entities
- https://core.telegram.org/method/messages.deleteMessages, /method/messages.editMessage, /method/messages.forwardMessages
- https://core.telegram.org/bots/features and https://core.telegram.org/bots/faq
- https://github.com/telegramdesktop/tdesktop/issues/25829 and /27189
- https://bugs.telegram.org/c/12370, /c/203, /c/22902, /c/9153, /c/23281
- https://www.pie-menu.com/shortcuts/telegram and https://usethekeyboard.com/telegram/
- https://www.reddit.com/r/matrixdotorg/comments/1abd6bf/is_there_a_time_limit_for_the_edit_message/
- https://stackoverflow.com/questions/55991350/telegram-bot-cant-delete-messages-that-was-sent-48h-ago-despite-of-it-has-permi
- https://www.facebook.com/fb-answers/turn-off-read-receipts-telegram/
