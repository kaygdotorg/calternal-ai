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
  unread: = count(records after state.read through)
records:
  quote: link(chats records)
  mentions: people
  attachments: links(any)
state:
  read through: record anchor
  pinned: yes/no, default no
  muted: yes/no, default no
  draft: long text
row: title, people, unread
page: records(quote, mentions, attachments)
operations:
  add: title
  change: only archived
  rename: code, menu
    inputs:
      item: link(chats), required
      title: text, required
  send: code
    inputs:
      item: link(chats), required
      text: long text, required
      quote: link(chats records)
      mentions: people
      attachments: links(any)
      harness: text
  change record: quote, mentions, attachments, within 48h
  remove record
  mark read: set state.read through = ask
  pin: set state.pinned = yes, when not state.pinned, menu
  unpin: set state.pinned = no, when state.pinned, menu
  mute: set state.muted = yes, when not state.muted, menu
  unmute: set state.muted = no, when state.muted, menu
---
Keep messages as records in the chat. Use the listed operations to send a
message or change a chat.
