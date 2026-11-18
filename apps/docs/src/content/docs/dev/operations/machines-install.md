---
title: "machines.install"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Create a single-use personal machine enrollment script

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "machines.install",
  "description": "Create a single-use personal machine enrollment script",
  "mutates": true
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
  "description": "The enrollment script and the deadline for redeeming its one-time code.",
  "properties": {
    "expires_ms": {
      "description": "Unix milliseconds when the code expires if unused.",
      "format": "int64",
      "type": "integer"
    },
    "script": {
      "description": "Readable shell script containing the private enrollment code.",
      "type": "string"
    }
  },
  "required": [
    "expires_ms",
    "script"
  ],
  "title": "MachineInstallResult",
  "type": "object"
}
```
