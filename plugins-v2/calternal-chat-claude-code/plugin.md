---
plugin: calternal-chat-claude-code
name: Claude Code
icon: terminal
version: 2.0.0
format: 1
programs:
  calternal-provider-claude-code: executable programs/calternal-provider-claude-code
calls: calternal-chat.send
sends out: api.anthropic.com
fills:
  calternal-chat:harness: operation start, as "Claude Code"
---
Run Claude Code on request. The run sends the chat text it may read to
Anthropic and writes its finished reply to the chat.
