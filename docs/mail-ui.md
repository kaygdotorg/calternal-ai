# Mail interface

Lane `lane/mail-ui`, 2026-11-18. Reading first; nothing is ever sent. The screens run on a fixture mailbox
(`apps/web/src/apps/mail/fixtures.ts`), not a real server: the web client cannot yet reach the plugin's operations.

## What exists

Mail is an app in the shell like Notes and Threads (`apps/web/src/apps/mail`, registered in `registry.ts`):

| Region | Content | Drawn by |
| --- | --- | --- |
| Sidebar | Account selector (only with several accounts), folder selector, headers with an Unread badge, "Older messages", "Add an account" | GenUI from `listView` |
| Middle card, `/mail/<uid>` | Subject as the page heading (feeds the sticky header pill), sender, date and size, plain text, attachment table, a note about HTML and remote images | App heading plus GenUI from `messageView` |
| Middle card, `/mail/setup` | Server, port and username, then a statement that the password is asked for in the protected prompt | GenUI from `setupView` |

The screens are pure functions in `plugins/calternal-mail/src/views.ts` (exported as `@calternal/mail/views`) from the operations'
result shapes to GenUI source. Mail text only ever enters a description through `lit`, which escapes every character that could end
a string or start a template, so message content cannot become code. Sender and date go through a Text node, which is Markdown,
so `plain` first removes the characters that could form a link or an image.

**How a screen talks back.** GenUI has no action that invokes an operation. Each screen declares `$go` and its buttons `Set` it to
a command (`open:<uid>`, `more`, `setup`, `add`, `back`); the selectors bind `$account` and `$folder`. `MailStore.apply` reads the state
after each change and performs the operation. The form's three fields are `$host`, `$port`, `$username`. There is no password field.

Marking read happens when a message is opened, after it is shown, and does not block showing it; the list's badge follows when
the server confirms. A mailbox change (`uid_validity` differs) says "Refresh the folder".

## Not done, and why

- No real server: the fixture client implements `MailClient`, which mirrors the operations' shapes. A server client is the next step and needs the operation route for plugin operations.
- The protected password prompt is unwired for Mail: the shell's secret prompt belongs to the Threads request bar. After adding an account the screen says it waits for the password, and the account lists nothing yet.
- The sealed HTML frame is not drawn (see gap 2). The reading pane shows the plain text and says there is an HTML version.
- Remote images load only on an explicit authenticated POST to the frame URL from `plugins.network.content.get`, even under Always; Never keeps them off (plugins/calternal-mail/README.md). That request belongs to the sealed frame, so the pane cannot offer it: a message with HTML says images are not loaded on their own. The plugin returns no remote-image flag, so the note shows for every message with HTML.
- Not checked on a phone or in Safari on the iPhone Simulator; see the lane issue.

## GenUI gaps

What the design needs that the description language or the shell's renderer cannot say. Each was met with the nearest component, and
nothing was added to the shell to work around it.

| # | Need | Used instead |
| --- | --- | --- |
| 1 | An action that invokes an operation (and passes its result back) | `Set` of a `$go` command that the client reads: the state is the interface |
| 2 | A sealed frame (`frame_id`, the empty-sandbox iframe the plugin guide describes) and its "Load remote images" action (authenticated POST to the frame URL) | `Code` with the plain text, and a note |
| 3 | A protected secret prompt, bound to a plugin's pending request | A note; the prompt itself is outside GenUI by design |
| 4 | A list row: leading mark, title, sub line, trailing date, long-press menu, swipe, selected state, `Row` link semantics | `Stack` of `Button` (selected is an "Open" `Badge`), `Badge`, `Text`; no long-press |
| 5 | A page title for the header pill (`data-chrome-title`) and a sticky header | The app draws the heading around the description |
| 6 | Paging a long list (reach-to-load, virtual rows) | An "Older messages" `Button`; every header stays in the page |
| 7 | Wrapped plain text with preserved line breaks | `Code` (monospace, scrolls sideways). `Text` is Markdown and would reflow, link and fetch images |
| 8 | A phone-width table | `Table` (no wrap, scrolls sideways) for attachments |
| 9 | The app's `Menu` for many choices | `Select` (a native control) when there are more than five folders; `Choice` (the shared Selector) otherwise |
| 10 | Icons, grids and tabs | The shell's renderer does not draw `Icon`, `Tabs`, `List` or `Grid` yet, so they are not used |

Data-layer gaps found along the way, outside GenUI: headers carry bare addresses with no display name, no preview line, no
recipient and no unread count; there is no remote-image flag; there is no operation that returns a view, so the manifest declares
no generated-view slot and the screens ship as a module.

## Simplification opportunities

See the lane issue.
