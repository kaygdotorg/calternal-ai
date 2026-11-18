---
title: "plugins.settings.set"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Edit a declared space plugin setting

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "plugins.settings.set",
  "description": "Edit a declared space plugin setting",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Edit one setting while retaining edits from other devices.",
  "properties": {
    "base": {
      "description": "Override last read for this key. Null means no override."
    },
    "key": {
      "description": "Stable setting key from that package's manifest.",
      "type": "string"
    },
    "plugin_id": {
      "description": "Installed package ID.",
      "type": "string"
    },
    "resolve": {
      "description": "Explicit conflict resolution echoes every alternative the caller saw.",
      "items": true,
      "type": [
        "array",
        "null"
      ]
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    },
    "value": {
      "description": "New override of the declared type. Null clears the override."
    }
  },
  "required": [
    "base",
    "key",
    "plugin_id",
    "space_id",
    "value"
  ],
  "title": "PluginSettingEdit",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Durable receipt and scoped invalidation for a setting write.",
  "properties": {
    "conflicted": {
      "description": "True when manual resolution is needed.",
      "type": "boolean"
    },
    "key": {
      "description": "Changed manifest key.",
      "type": "string"
    },
    "plugin_id": {
      "description": "Package whose setting should be re-read.",
      "type": "string"
    }
  },
  "required": [
    "conflicted",
    "key",
    "plugin_id"
  ],
  "title": "PluginSettingChange",
  "type": "object"
}
```
