---
title: "requests.deny"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Decline a pending approval

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "requests.deny",
  "description": "Decline a pending approval",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Decline one pending approval.",
  "properties": {
    "id": {
      "description": "Approval UUID.",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "RequestDenyInput",
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
