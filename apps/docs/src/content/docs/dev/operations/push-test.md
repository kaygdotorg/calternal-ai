---
title: "push.test"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Queue a test push to this browser

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "external",
  "id": "push.test",
  "description": "Queue a test push to this browser",
  "mutates": true
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
  "description": "Best-effort admission of a test notification for the current installation.",
  "properties": {
    "queued": {
      "description": "True when the background worker accepted the test.",
      "type": "boolean"
    }
  },
  "required": [
    "queued"
  ],
  "title": "PushQueued",
  "type": "object"
}
```
