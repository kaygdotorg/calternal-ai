---
title: "machines.onboarding"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Check whether the installation has an enrolled, available machine

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "machines.onboarding",
  "description": "Check whether the installation has an enrolled, available machine",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "No caller-controlled scope or target for machine onboarding and enrollment.",
  "title": "MachineOnboardingInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Enrollment and current placement availability for first-owner setup.",
  "properties": {
    "attached": {
      "description": "An active enrollment exists on this installation.",
      "type": "boolean"
    },
    "machine_id": {
      "description": "The selected machine UUID for the owner's explicit verification.",
      "type": [
        "string",
        "null"
      ]
    },
    "ready": {
      "description": "The selected machine is connected and can accept a sandbox placement.",
      "type": "boolean"
    }
  },
  "required": [
    "attached",
    "ready"
  ],
  "title": "MachineOnboardingResult",
  "type": "object"
}
```
