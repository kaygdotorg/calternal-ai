---
title: "machines.verify"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Verify a selected machine and provider sign-in

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "machines.verify",
  "description": "Verify a selected machine and provider sign-in",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Select the space for the diagnostic job and the exact enrolled machine.",
  "properties": {
    "machine_id": {
      "description": "Enrolled machine UUID. Pool machines require server settings authority.",
      "type": "string"
    },
    "provider_id": {
      "default": "",
      "description": "Optional provider plugin. An empty or absent value checks the installation's default provider.",
      "type": "string"
    },
    "space_id": {
      "description": "A space where the caller can use plugins.",
      "type": "string"
    }
  },
  "required": [
    "machine_id",
    "space_id"
  ],
  "title": "MachineVerifyInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "ProviderCheck": {
      "additionalProperties": false,
      "description": "A bounded observation of the official provider program's own check.",
      "properties": {
        "installed": {
          "description": "The provider program was found inside the machine's execution environment.",
          "type": "boolean"
        },
        "sign_in_command": {
          "description": "The program's own sign-in command, for display only.",
          "type": "string"
        },
        "signed_in": {
          "description": "The program reported an active login for this account.",
          "type": "boolean"
        }
      },
      "required": [
        "installed",
        "sign_in_command",
        "signed_in"
      ],
      "type": "object"
    }
  },
  "description": "The bounded result of one diagnostic. No job IDs or machine logs are returned.",
  "properties": {
    "provider": {
      "anyOf": [
        {
          "$ref": "#/definitions/ProviderCheck"
        },
        {
          "type": "null"
        }
      ],
      "description": "The provider's check, absent if it did not report an outcome."
    },
    "sandbox_started": {
      "description": "Whether the machine confirmed that its execution environment started.",
      "type": "boolean"
    }
  },
  "required": [
    "sandbox_started"
  ],
  "title": "MachineVerifyResult",
  "type": "object"
}
```
