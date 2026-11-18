---
title: "automations.next_run"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read the next scheduled run

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "space",
  "id": "automations.next_run",
  "description": "Read the next scheduled run",
  "mutates": false
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
  "description": "Next scheduled instant for an automation.",
  "properties": {
    "next_run_ms": {
      "description": "UTC epoch milliseconds, or null when paused or stopped.",
      "format": "int64",
      "type": [
        "integer",
        "null"
      ]
    }
  },
  "title": "AutomationNext",
  "type": "object"
}
```
