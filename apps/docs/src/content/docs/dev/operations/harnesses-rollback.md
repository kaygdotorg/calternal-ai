---
title: "harnesses.rollback"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Restore a previously tested harness runtime

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "harnesses.rollback",
  "description": "Restore a previously tested harness runtime",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Select a tested runtime or restore the previous active runtime.",
  "properties": {
    "id": {
      "description": "Installed provider package identifier.",
      "type": "string"
    },
    "image": {
      "description": "Exact immutable image ID observed in the list.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "image"
  ],
  "title": "HarnessSelect",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Immutable artifacts for an operator-reviewed provider runtime. The binaries run on the enrolled machine, inside the thread's sandbox.",
  "properties": {
    "guest_image_digest": {
      "description": "Verified guest-image manifest digest, when a MicroVM image is staged.",
      "type": [
        "string",
        "null"
      ]
    },
    "image": {
      "description": "Full local OCI image ID, including its sha256 prefix.",
      "type": "string"
    },
    "interactive": {
      "description": "The adapter accepts stop, steer and approval responses during a turn.",
      "type": "boolean"
    },
    "package_sha256": {
      "description": "Integrity digest of the CLI artifact for this deployment.",
      "type": "string"
    },
    "version": {
      "description": "Exact official CLI version reported by the adapter.",
      "type": "string"
    }
  },
  "required": [
    "image",
    "package_sha256",
    "version"
  ],
  "title": "HarnessPin",
  "type": "object"
}
```
