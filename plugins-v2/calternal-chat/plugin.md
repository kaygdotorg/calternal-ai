---
plugin: calternal-chat
name: Chat
icon: chat
version: 2.0.0
format: 1
sends out: nothing
slots:
  composer: single, gives "the draft and the open chat", picked by person, recommended calternal-chat
  harness: single, gives "the open chat for the requested run", picked by person, recommended calternal-chat-claude-code
fills:
  calternal-chat:composer: operation send
---
Keep conversations as Markdown. Install a harness to talk to an agent.
