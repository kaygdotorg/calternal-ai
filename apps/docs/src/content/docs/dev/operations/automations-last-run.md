---
title: "automations.last_run"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read the latest automation outcome

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "space",
  "id": "automations.last_run",
  "description": "Read the latest automation outcome",
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
  "definitions": {
    "AutomationOutcome": {
      "description": "One scheduling outcome, with a thread when a turn was admitted.",
      "properties": {
        "at_ms": {
          "description": "Time this outcome was recorded, in UTC epoch milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "reason": {
          "description": "Caller-safe explanation for an exceptional outcome.",
          "type": [
            "string",
            "null"
          ]
        },
        "scheduled_at_ms": {
          "description": "Scheduled instant in UTC epoch milliseconds, absent for manual runs.",
          "format": "int64",
          "type": [
            "integer",
            "null"
          ]
        },
        "status": {
          "description": "Stable run lifecycle status.",
          "type": "string"
        },
        "thread_id": {
          "description": "Admitted thread UUID, if any.",
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "at_ms",
        "status"
      ],
      "type": "object"
    }
  },
  "description": "Latest recorded run or skip.",
  "properties": {
    "last": {
      "anyOf": [
        {
          "$ref": "#/definitions/AutomationOutcome"
        },
        {
          "type": "null"
        }
      ],
      "description": "Null before the first outcome."
    }
  },
  "title": "AutomationLast",
  "type": "object"
}
```
