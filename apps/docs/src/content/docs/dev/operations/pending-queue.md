---
title: "pending.queue"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

pending queue

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "pending.queue",
  "description": "pending queue",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "OperationRequest": {
      "additionalProperties": false,
      "description": "Envelope for every `POST /api/v1/operations` call. Retain request identity and input unchanged after uncertain mutation outcomes.",
      "properties": {
        "input": {
          "additionalProperties": true,
          "description": "Operation-specific JSON object; must pass the selected operation schema before dispatch.",
          "type": "object"
        },
        "operation": {
          "description": "Versioned operation identifier; operation envelopes and actions require 1..128 UTF-8 bytes.",
          "type": "string"
        },
        "request_id": {
          "description": "Caller correlation/retry identity, 1..128 Unicode scalars; preserve it with identical input on retry.",
          "maxLength": 128,
          "minLength": 1,
          "type": "string"
        }
      },
      "required": [
        "input",
        "operation",
        "request_id"
      ],
      "type": "object"
    }
  },
  "description": "Queue or replace a pending write before its first submission attempt.",
  "properties": {
    "created_at_ms": {
      "description": "Original offline creation time in Unix milliseconds; future times are refused.",
      "format": "int64",
      "type": "integer"
    },
    "item_id": {
      "description": "Full target item UUID, used to clear its draft after a successful send.",
      "type": "string"
    },
    "request": {
      "allOf": [
        {
          "$ref": "#/definitions/OperationRequest"
        }
      ],
      "description": "Validated operation envelope with a stable request ID; nested input operations forbidden."
    },
    "space_id": {
      "description": "Owning space UUID; must agree with the write's resolved target.",
      "type": "string"
    }
  },
  "required": [
    "created_at_ms",
    "item_id",
    "request",
    "space_id"
  ],
  "title": "PendingQueueInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "AccountInputRecord": {
      "additionalProperties": false,
      "description": "Current account-owned record; revisions come from the server, never a device clock.",
      "properties": {
        "created_at_ms": {
          "description": "Original creation time, preserved on edits.",
          "format": "int64",
          "type": "integer"
        },
        "id": {
          "description": "Draft item UUID or pending request ID.",
          "type": "string"
        },
        "item_id": {
          "description": "Thread or item being composed against.",
          "type": "string"
        },
        "request": {
          "anyOf": [
            {
              "$ref": "#/definitions/OperationRequest"
            },
            {
              "type": "null"
            }
          ],
          "description": "Stable operation, absent for drafts."
        },
        "revision": {
          "description": "Opaque server revision for this replacement.",
          "type": "string"
        },
        "space_id": {
          "description": "Space checked before delivery or submission.",
          "type": "string"
        },
        "state": {
          "description": "queued, confirmation_required, or submitting (immutable after attempt).",
          "type": "string"
        },
        "text": {
          "description": "Draft text, absent for pending submissions.",
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "created_at_ms",
        "id",
        "item_id",
        "revision",
        "space_id",
        "state"
      ],
      "type": "object"
    },
    "OperationRequest": {
      "additionalProperties": false,
      "description": "Envelope for every `POST /api/v1/operations` call. Retain request identity and input unchanged after uncertain mutation outcomes.",
      "properties": {
        "input": {
          "additionalProperties": true,
          "description": "Operation-specific JSON object; must pass the selected operation schema before dispatch.",
          "type": "object"
        },
        "operation": {
          "description": "Versioned operation identifier; operation envelopes and actions require 1..128 UTF-8 bytes.",
          "type": "string"
        },
        "request_id": {
          "description": "Caller correlation/retry identity, 1..128 Unicode scalars; preserve it with identical input on retry.",
          "maxLength": 128,
          "minLength": 1,
          "type": "string"
        }
      },
      "required": [
        "input",
        "operation",
        "request_id"
      ],
      "type": "object"
    }
  },
  "description": "Operation result containing one private input, or none after removal/send.",
  "properties": {
    "record": {
      "anyOf": [
        {
          "$ref": "#/definitions/AccountInputRecord"
        },
        {
          "type": "null"
        }
      ],
      "description": "Current record; confirmation_required is also a request addressed to this person."
    },
    "result": {
      "description": "Underlying write's ordinary receipt, returned after submission."
    }
  },
  "title": "AccountInputResult",
  "type": "object"
}
```
