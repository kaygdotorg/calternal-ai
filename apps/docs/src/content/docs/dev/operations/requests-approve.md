---
title: "requests.approve"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Approve with a fresh verified passkey

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "widens_access",
  "id": "requests.approve",
  "description": "Approve with a fresh verified passkey",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "ApprovalLifetime": {
      "description": "How long approved authority remains usable.",
      "oneOf": [
        {
          "description": "The first matching action may use the grant.",
          "enum": [
            "once"
          ],
          "type": "string"
        },
        {
          "description": "The grant survives turns until the thread is archived or deleted.",
          "enum": [
            "this_thread"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "Resolve a pending approval with the selected lifetime.",
  "properties": {
    "id": {
      "description": "Approval UUID.",
      "type": "string"
    },
    "lifetime": {
      "allOf": [
        {
          "$ref": "#/definitions/ApprovalLifetime"
        }
      ],
      "description": "Maximum duration of the grant."
    }
  },
  "required": [
    "id",
    "lifetime"
  ],
  "title": "RequestApproveInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Result of resolving a private request.",
  "properties": {
    "id": {
      "description": "Resolved request UUID.",
      "type": "string"
    },
    "status": {
      "description": "Answered, approved or denied.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "status"
  ],
  "title": "RequestResolution",
  "type": "object"
}
```
