---
title: "sharing.shared_with_me"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List items shared with the signed-in account

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "sharing.shared_with_me",
  "description": "List items shared with the signed-in account",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "List current received shares for an app or collection kind.",
  "properties": {
    "cursor": {
      "description": "An optional continuation issued for this operation and scope.",
      "type": [
        "string",
        "null"
      ]
    },
    "kind": {
      "description": "The item kind, either notes or collections in this slice.",
      "type": "string"
    }
  },
  "required": [
    "kind"
  ],
  "title": "SharesReceivedInput",
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
