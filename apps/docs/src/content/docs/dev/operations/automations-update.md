---
title: "automations.update"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Update your automation at a known revision

Capability: content.write.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "content.write",
  "target": "space",
  "id": "automations.update",
  "description": "Update your automation at a known revision",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "AgentMode": {
      "description": "Thread permission mode; every change revokes the previous thread token.",
      "oneOf": [
        {
          "description": "Thread, workspace and originating app only; other scopes require approval.",
          "enum": [
            "least_access"
          ],
          "type": "string"
        },
        {
          "description": "Current space access, reviewed by rules; high-risk classes require approval.",
          "enum": [
            "approve_for_me"
          ],
          "type": "string"
        },
        {
          "description": "Current space access without approvals, independently of sandboxing.",
          "enum": [
            "full_access"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "Change selected fields, sending all three preview fields to change the schedule.",
  "properties": {
    "base_revision": {
      "description": "Opaque current definition revision.",
      "type": "string"
    },
    "id": {
      "description": "Stable automation UUID.",
      "type": "string"
    },
    "machine_id": {
      "default": null,
      "description": "Machine UUID, or an empty string to select the authorized sandbox.",
      "type": [
        "string",
        "null"
      ]
    },
    "mode": {
      "anyOf": [
        {
          "$ref": "#/definitions/AgentMode"
        },
        {
          "type": "null"
        }
      ],
      "default": null,
      "description": "Mode to apply to subsequent runs."
    },
    "name": {
      "default": null,
      "description": "New display title.",
      "type": [
        "string",
        "null"
      ]
    },
    "prompt": {
      "default": null,
      "description": "New Markdown prompt.",
      "type": [
        "string",
        "null"
      ]
    },
    "schedule": {
      "default": null,
      "description": "Canonical expression for the same schedule preview.",
      "type": [
        "string",
        "null"
      ]
    },
    "schedule_text": {
      "default": null,
      "description": "Original words for a changed or reconfirmed schedule.",
      "type": [
        "string",
        "null"
      ]
    },
    "space_id": {
      "description": "Owning space UUID.",
      "type": "string"
    },
    "timezone": {
      "default": null,
      "description": "Account zone confirmed by that preview.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "base_revision",
    "id",
    "space_id"
  ],
  "title": "AutomationUpdate",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "AgentMode": {
      "description": "Thread permission mode; every change revokes the previous thread token.",
      "oneOf": [
        {
          "description": "Thread, workspace and originating app only; other scopes require approval.",
          "enum": [
            "least_access"
          ],
          "type": "string"
        },
        {
          "description": "Current space access, reviewed by rules; high-risk classes require approval.",
          "enum": [
            "approve_for_me"
          ],
          "type": "string"
        },
        {
          "description": "Current space access without approvals, independently of sandboxing.",
          "enum": [
            "full_access"
          ],
          "type": "string"
        }
      ]
    },
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
    },
    "AutomationRecord": {
      "description": "An automation's status and saved definition metadata.",
      "properties": {
        "account_timezone": {
          "description": "Creator's new account zone, when a schedule move requires confirmation.",
          "type": [
            "string",
            "null"
          ]
        },
        "enabled": {
          "description": "Whether scheduled execution is enabled.",
          "type": "boolean"
        },
        "human": {
          "description": "Normalized schedule in plain words.",
          "type": "string"
        },
        "id": {
          "description": "Stable automation UUID.",
          "type": "string"
        },
        "last": {
          "anyOf": [
            {
              "$ref": "#/definitions/AutomationOutcome"
            },
            {
              "type": "null"
            }
          ],
          "description": "Most recent recorded outcome, absent before the first occurrence."
        },
        "machine_id": {
          "description": "Explicit machine when visible to the creator.",
          "type": [
            "string",
            "null"
          ]
        },
        "mode": {
          "allOf": [
            {
              "$ref": "#/definitions/AgentMode"
            }
          ],
          "description": "Permission mode for subsequent runs."
        },
        "name": {
          "description": "Display title.",
          "type": "string"
        },
        "next_run_ms": {
          "description": "Next instant in UTC epoch milliseconds, when scheduled.",
          "format": "int64",
          "type": [
            "integer",
            "null"
          ]
        },
        "path": {
          "description": "Relative Markdown path in the owning space.",
          "type": "string"
        },
        "prompt": {
          "description": "Exact prompt supplied with individual reads and definition edits.",
          "type": [
            "string",
            "null"
          ]
        },
        "revision": {
          "description": "Opaque current definition revision.",
          "type": "string"
        },
        "schedule": {
          "description": "Canonical calendar expression generated by the server.",
          "type": "string"
        },
        "schedule_text": {
          "description": "Original schedule phrase, including its original spelling and whitespace.",
          "type": "string"
        },
        "stop_reason": {
          "description": "Last stop reason, if automatic execution was revoked.",
          "type": [
            "string",
            "null"
          ]
        },
        "timezone": {
          "description": "IANA zone used to compute the next instant.",
          "type": "string"
        }
      },
      "required": [
        "enabled",
        "human",
        "id",
        "mode",
        "name",
        "path",
        "revision",
        "schedule",
        "schedule_text",
        "timezone"
      ],
      "type": "object"
    }
  },
  "description": "A single automation definition.",
  "properties": {
    "automation": {
      "allOf": [
        {
          "$ref": "#/definitions/AutomationRecord"
        }
      ],
      "description": "Definition and latest scheduling status."
    }
  },
  "required": [
    "automation"
  ],
  "title": "AutomationResult",
  "type": "object"
}
```
