---
title: "agent.settings"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read or set your account defaults or thread retry settings

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "agent.settings",
  "description": "Read or set your account defaults or thread retry settings",
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
    },
    "AgentRetrySettings": {
      "additionalProperties": false,
      "description": "Typed automatic provider recovery settings; manifest defaults apply per account and a manual turn may override them. Neither permits machine re-placement.",
      "properties": {
        "provider_errors": {
          "description": "Retry explicit transient provider errors, with no pending tool effect.",
          "type": "boolean"
        },
        "subscription_limits": {
          "description": "Resume subscription usage limits at the reset reported by the provider.",
          "type": "boolean"
        }
      },
      "required": [
        "provider_errors",
        "subscription_limits"
      ],
      "type": "object"
    },
    "ProviderContext": {
      "additionalProperties": false,
      "description": "A labelled contribution; data, never authority or system instructions.",
      "properties": {
        "source": {
          "description": "Plugin or attachment label, at most 128 UTF-8 bytes.",
          "type": "string"
        },
        "text": {
          "description": "Contribution text; included in the command's total bound.",
          "type": "string"
        }
      },
      "required": [
        "source",
        "text"
      ],
      "type": "object"
    }
  },
  "description": "Input union for manual-turn operations. Each operation rejects irrelevant fields. UUIDs are full resource identifiers; model work never starts without a space check.",
  "properties": {
    "after": {
      "default": 0,
      "description": "Next job sequence requested; zero on first read.",
      "format": "uint64",
      "minimum": 0,
      "type": "integer"
    },
    "app": {
      "default": "",
      "description": "Requested app prefix for an access request.",
      "type": "string"
    },
    "context": {
      "default": [],
      "description": "Labelled context contributions, bounded by the provider command budget.",
      "items": {
        "$ref": "#/definitions/ProviderContext"
      },
      "type": "array"
    },
    "direct": {
      "default": false,
      "description": "Whether an account-owned machine runs directly; pool machines refuse this.",
      "type": "boolean"
    },
    "epoch": {
      "default": "",
      "description": "Stream epoch last seen, empty on first read.",
      "type": "string"
    },
    "job_id": {
      "default": "",
      "description": "Job UUID for job operations. A thread credential can omit it for access.request.",
      "type": "string"
    },
    "lease_id": {
      "default": "",
      "description": "Fenced lease UUID for explicit machine-level resumption.",
      "type": "string"
    },
    "machine_id": {
      "default": "",
      "description": "Machine UUID for owner-controlled execution settings.",
      "type": "string"
    },
    "mode": {
      "allOf": [
        {
          "$ref": "#/definitions/AgentMode"
        }
      ],
      "default": "approve_for_me",
      "description": "Permission mode; defaults to Approve for me."
    },
    "model": {
      "default": "",
      "description": "Official CLI model; empty uses the selected provider manifest default.",
      "type": "string"
    },
    "options": {
      "additionalProperties": true,
      "default": {},
      "description": "Options declared by the selected provider, bounded by the command budget.",
      "type": "object"
    },
    "personal_machine": {
      "description": "Use the caller's machine and login for this turn in a configured shared space.",
      "type": "boolean"
    },
    "pool": {
      "default": false,
      "description": "Check an entitled pool machine without exposing its identity.",
      "type": "boolean"
    },
    "prompt": {
      "default": "",
      "description": "Person's prompt; at most 16 KiB, required for run.",
      "type": "string"
    },
    "provider_id": {
      "default": "",
      "description": "Enabled provider plugin; empty selects calternal-claude-code.",
      "type": "string"
    },
    "retry_failed": {
      "default": false,
      "description": "Explicit retry of a failed workspace discard; never implied by status polling.",
      "type": "boolean"
    },
    "retry_settings": {
      "anyOf": [
        {
          "$ref": "#/definitions/AgentRetrySettings"
        },
        {
          "type": "null"
        }
      ],
      "default": null,
      "description": "Per-thread recovery override; absent uses the account defaults."
    },
    "source_app": {
      "default": "",
      "description": "Originating app operation prefix; empty grants no originating-app access.",
      "type": "string"
    },
    "space_id": {
      "default": "",
      "description": "Owning space UUID, required for check/run, otherwise derived from the job.",
      "type": "string"
    },
    "thread_id": {
      "default": "",
      "description": "Thread UUID for check/run; no thread file is created in step one.",
      "type": "string"
    },
    "write": {
      "default": false,
      "description": "Whether the requested grant allows mutations as well as reads.",
      "type": "boolean"
    }
  },
  "title": "AgentInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": true,
  "title": "Map_of_AnyValue",
  "type": "object"
}
```
