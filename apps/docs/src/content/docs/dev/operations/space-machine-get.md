---
title: "space.machine.get"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read the execution setting for a space

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "space.machine.get",
  "description": "Read the execution setting for a space",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for reading the execution setting.",
  "properties": {
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceMachineGet",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Current space execution setting and the notice shown during setup.",
  "properties": {
    "configured": {
      "description": "Whether a shared machine is configured.",
      "type": "boolean"
    },
    "machine_id": {
      "description": "Selected machine UUID, visible only with permission to change it.",
      "type": [
        "string",
        "null"
      ]
    },
    "notice": {
      "description": "Notice for the person configuring the provider account.",
      "type": "string"
    }
  },
  "required": [
    "configured",
    "notice"
  ],
  "title": "SpaceMachineResult",
  "type": "object"
}
```
