---
title: "push.devices"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List your registered push devices and the public VAPID key

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "push.devices",
  "description": "List your registered push devices and the public VAPID key",
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
    "PushDevice": {
      "description": "One active subscription, with no endpoint or browser key material.",
      "properties": {
        "current": {
          "description": "Whether the current credential belongs to this installation.",
          "type": "boolean"
        },
        "device_id": {
          "description": "Revocable installation UUID.",
          "type": "string"
        },
        "expires_at_ms": {
          "description": "Server-enforced expiry in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "name": {
          "description": "Display name from the installation directory.",
          "type": "string"
        }
      },
      "required": [
        "current",
        "device_id",
        "expires_at_ms",
        "name"
      ],
      "type": "object"
    }
  },
  "description": "Subscription inventory and the public browser application server key.",
  "properties": {
    "application_server_key": {
      "description": "Unpadded base64url VAPID public key for PushManager.subscribe.",
      "type": "string"
    },
    "devices": {
      "description": "The signed-in account's live subscriptions.",
      "items": {
        "$ref": "#/definitions/PushDevice"
      },
      "type": "array"
    }
  },
  "required": [
    "application_server_key",
    "devices"
  ],
  "title": "PushDevices",
  "type": "object"
}
```
