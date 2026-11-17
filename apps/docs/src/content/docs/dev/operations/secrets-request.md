---
title: "secrets.request"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Ask for a secret; a person supplies the value through the trusted prompt

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "secrets.request",
  "description": "Ask for a secret; a person supplies the value through the trusted prompt",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Ask for a secret; the value is supplied later through the trusted surface.",
  "properties": {
    "label": {
      "description": "Short label shown in the prompt, at most 120 characters.",
      "type": "string"
    },
    "name": {
      "description": "Secret name: 1–96 bytes of `a-z 0-9 . _ - :`, starting with a letter or digit.",
      "type": "string"
    },
    "reason": {
      "description": "Why it is needed, shown in the prompt, at most 500 characters.",
      "type": "string"
    },
    "space_id": {
      "description": "Owning space UUID; omit for a personal (account-owned) secret.",
      "type": "string"
    }
  },
  "required": [
    "label",
    "name",
    "reason"
  ],
  "title": "SecretsRequestInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Pending request identifier returned by `secrets.request`.",
  "properties": {
    "request_id": {
      "description": "UUID of the pending request, answered by the provide endpoint.",
      "type": "string"
    }
  },
  "required": [
    "request_id"
  ],
  "title": "SecretsRequestResult",
  "type": "object"
}
```
