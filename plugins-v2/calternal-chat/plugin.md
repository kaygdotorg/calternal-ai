---
plugin: calternal-chat
name: Chat
icon: chat
version: 2.0.0
format: 1
sends out: nothing
slots:
  composer: single, gives text text, draft state.draft, quote quote, picked by person, recommended calternal-chat, inputs chats.send, label "Message"
  harness: single, gives "the open chat for the requested run", picked by person, recommended calternal-chat-claude-code, label "Ask an agent"
fills:
  calternal-chat:composer: operation send
---
Keep conversations as Markdown. Install a harness to talk to an agent.
