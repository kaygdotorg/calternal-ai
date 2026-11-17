---
title: "pending.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

pending list

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "pending.list",
  "description": "pending list",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Account-private list input. No caller-supplied account is accepted.",
  "title": "AccountInputList",
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
  "description": "Complete bounded account input snapshot, fetched again after a stream reset.",
  "properties": {
    "records": {
      "description": "Current authorized records, ordered by identity.",
      "items": {
        "$ref": "#/definitions/AccountInputRecord"
      },
      "type": "array"
    }
  },
  "required": [
    "records"
  ],
  "title": "AccountInputListResult",
  "type": "object"
}
```
