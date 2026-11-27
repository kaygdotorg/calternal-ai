---
kind: runs
name: Claude Code run
format: 1
folder: Claude Code Runs
hidden: keep 7d
fields:
  chat: link(calternal-chat.chats), required
operations:
  start: code
    inputs:
      chat: link(calternal-chat.chats), required
      prompt: link(calternal-chat.chats records), required
---
Entry point for starting Claude Code in the selected chat. The host owns
the live run and its log.
