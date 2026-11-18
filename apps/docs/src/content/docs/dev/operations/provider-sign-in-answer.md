---
title: "provider.sign_in.answer"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Supply a code to the official provider login

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "provider.sign_in.answer",
  "description": "Supply a code to the official provider login",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Supply a browser authorization code to the original official process.",
  "properties": {
    "code": {
      "description": "The short-lived code. Never saved in a request receipt.",
      "type": "string"
    },
    "id": {
      "description": "Account-private request UUID.",
      "type": "string"
    }
  },
  "required": [
    "code",
    "id"
  ],
  "title": "ProviderSignInAnswer",
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
