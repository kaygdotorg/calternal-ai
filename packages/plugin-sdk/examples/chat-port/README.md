# Chat plugin, ported to the v2 SDK

A copy of `plugins/calternal-chat` and `plugins/calternal-chat-claude-code`
from `lane/threads-ui` (commit 4e513af8), ported step by step to the SDK's
fake host, `defineOperation`, typed context, broker helpers, limits and
`calternal-plugin build`. It proves those SDK pieces against a real plugin.

The interface lane owns the description files and the integration lane owns
the worker code; they adopt the diff on issues #99 and #101. Delete this copy
once they have. Not a workspace: run its scripts from this directory.
