---
title: "space.machine.set"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Set the shared execution machine for a space

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "widens_access",
  "id": "space.machine.set",
  "description": "Set the shared execution machine for a space",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for setting the shared execution machine.",
  "properties": {
    "machine_id": {
      "description": "Owner's sandbox machine UUID or null to clear the setting.",
      "type": "string"
    },
    "space_id": {
      "description": "Shared space UUID.",
      "type": "string"
    }
  },
  "required": [
    "machine_id",
    "space_id"
  ],
  "title": "SpaceMachineSet",
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
