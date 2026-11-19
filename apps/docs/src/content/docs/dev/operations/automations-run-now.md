---
title: "automations.run_now"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Start a thread for your automation

Capability: content.write.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "content.write",
  "target": "space",
  "id": "automations.run_now",
  "description": "Start a thread for your automation",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Space and existing automation identity.",
  "properties": {
    "id": {
      "description": "Stable automation UUID.",
      "type": "string"
    },
    "space_id": {
      "description": "Owning space UUID.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "space_id"
  ],
  "title": "AutomationTarget",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Admitted thread for a manual run.",
  "properties": {
    "thread_id": {
      "description": "Stable UUID of the new thread.",
      "type": "string"
    }
  },
  "required": [
    "thread_id"
  ],
  "title": "AutomationThread",
  "type": "object"
}
```
