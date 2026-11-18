---
title: "provider.sign_in.typing"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Offer provider re-authentication while composing

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "provider.sign_in.typing",
  "description": "Offer provider re-authentication while composing",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Report that the person began composing in a saved thread.",
  "properties": {
    "space_id": {
      "description": "Authorized space UUID.",
      "type": "string"
    },
    "thread_id": {
      "description": "Saved thread UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id",
    "thread_id"
  ],
  "title": "ProviderSignInTyping",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "anyOf": [
    {
      "$ref": "#/definitions/ProviderSignInResult"
    },
    {
      "type": "null"
    }
  ],
  "definitions": {
    "ProviderSignInResult": {
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
      "type": "object"
    }
  },
  "title": "Nullable_ProviderSignInResult"
}
```
