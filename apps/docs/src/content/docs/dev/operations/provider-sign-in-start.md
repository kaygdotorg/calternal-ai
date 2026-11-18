---
title: "provider.sign_in.start"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Start a private provider sign-in

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "provider.sign_in.start",
  "description": "Start a private provider sign-in",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Select the sandbox and official provider for sign-in.",
  "properties": {
    "machine_id": {
      "default": null,
      "description": "Explicitly selected personal machine UUID.",
      "type": [
        "string",
        "null"
      ]
    },
    "pool": {
      "default": false,
      "description": "Use an entitled anonymous pool machine. Mutually exclusive with machine_id.",
      "type": "boolean"
    },
    "provider_id": {
      "description": "Installed provider plugin ID.",
      "type": "string"
    },
    "shared": {
      "default": false,
      "description": "Sign in to the configured space login using space.settings authority.",
      "type": "boolean"
    },
    "space_id": {
      "description": "Space UUID in which the caller may use this provider.",
      "type": "string"
    },
    "thread_id": {
      "default": null,
      "description": "Optional saved thread UUID for the private prompt and final notice.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "provider_id",
    "space_id"
  ],
  "title": "ProviderSignInStart",
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
