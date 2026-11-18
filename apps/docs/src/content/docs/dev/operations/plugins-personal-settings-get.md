---
title: "plugins.personal_settings.get"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read your own declared plugin setting

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "plugins.personal_settings.get",
  "description": "Read your own declared plugin setting",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Address one manifest-declared setting in an accessible space.",
  "properties": {
    "key": {
      "description": "Stable setting key from that package's manifest.",
      "type": "string"
    },
    "plugin_id": {
      "description": "Installed package ID.",
      "type": "string"
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "key",
    "plugin_id",
    "space_id"
  ],
  "title": "PluginSettingAddress",
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
