---
kind: chats
name: Chat
format: 1
folder: Chats
item: folder with chat.md
editable: operations
template: conversation
fields:
  title: text, required
  people: people
  archived: yes/no, default no
  unread: = if(state.read through = empty, count(records), count(records, at > state.read through))
records:
  text: long text
  quote: link(chats records)
  mentions: people
  attachments: links(any)
state:
  read through: time
  pinned: yes/no, default no
  muted: yes/no, default no
  draft: long text
row: title, people, unread
page: records(text, quote, mentions, attachments)
operations:
  add: title
  change: only archived
  rename: code, menu
  send: code
  change record: text, quote, mentions, attachments, within 48h
  remove record
  mark read: set state.read through = ask
  pin: set state.pinned = yes, when not state.pinned, menu
  unpin: set state.pinned = no, when state.pinned, menu
  mute: set state.muted = yes, when not state.muted, menu
  unmute: set state.muted = no, when state.muted, menu
---
Keep messages as records in the chat. Use the listed operations to send a
message or change a chat.
