---
title: "provider.sign_in.status"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read a private provider sign-in result

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "provider.sign_in.status",
  "description": "Read a private provider sign-in result",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Look up one account-private, volatile sign-in request.",
  "properties": {
    "id": {
      "description": "Account-private request UUID.",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "ProviderSignInStatus",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "A sign-in result never includes the challenge or the browser's answer.",
  "properties": {
    "id": {
      "description": "Account-private request UUID.",
      "type": "string"
    },
    "status": {
      "description": "Pending, done or failed.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "status"
  ],
  "title": "ProviderSignInResult",
  "type": "object"
}
```
