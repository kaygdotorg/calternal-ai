---
title: "plugins.personal_settings.set"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Edit your own declared plugin setting

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "plugins.personal_settings.set",
  "description": "Edit your own declared plugin setting",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Replace the caller's override if their observed value still matches.",
  "properties": {
    "base": {
      "description": "Last observed override. Null means no override."
    },
    "key": {
      "description": "Personal setting key in that package's manifest.",
      "type": "string"
    },
    "plugin_id": {
      "description": "Installed package ID.",
      "type": "string"
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    },
    "value": {
      "description": "New override. Null restores the manifest default."
    }
  },
  "required": [
    "base",
    "key",
    "plugin_id",
    "space_id",
    "value"
  ],
  "title": "PluginPersonalSettingEdit",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Current values for a setting. The worker receives only its effective value.",
  "properties": {
    "alternatives": {
      "description": "Concurrent alternatives preserved until explicit resolution.",
      "items": true,
      "type": "array"
    },
    "effective": {
      "description": "Value the package sees, using the manifest default when needed."
    },
    "repair": {
      "description": "Caller-safe description of the file and line to repair, when applicable.",
      "type": [
        "string",
        "null"
      ]
    },
    "value": {
      "description": "Stored override for the next edit's base. Null indicates no override."
    },
    "writable": {
      "description": "True when the current settings file can be changed through the API.",
      "type": "boolean"
    }
  },
  "required": [
    "alternatives",
    "effective",
    "value",
    "writable"
  ],
  "title": "PluginSettingValue",
  "type": "object"
}
```
