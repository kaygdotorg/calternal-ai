---
title: "harnesses.stage"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Smoke-test a candidate harness runtime in a disposable sandbox

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "harnesses.stage",
  "description": "Smoke-test a candidate harness runtime in a disposable sandbox",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "HarnessOffer": {
      "additionalProperties": false,
      "description": "Public authentication instructions and deployment artifacts approved by the operator. The enrollment flow owns any credential bytes separately.",
      "properties": {
        "login_command": {
          "description": "Display-only command for manual authentication in the sandbox.",
          "type": "string"
        },
        "login_locations": {
          "description": "Exact HTTPS origins and paths emitted by the adapter, without queries.",
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "pin": {
          "allOf": [
            {
              "$ref": "#/definitions/HarnessPin"
            }
          ],
          "description": "Exact candidate the smoke test will run."
        }
      },
      "required": [
        "login_command",
        "login_locations",
        "pin"
      ],
      "type": "object"
    },
    "HarnessPin": {
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
      "type": "object"
    }
  },
  "description": "Smoke-test a candidate package runtime on a selected enrolled machine.",
  "properties": {
    "guest_machine_id": {
      "default": null,
      "description": "MicroVM machine to qualify when the candidate includes a guest digest.",
      "type": [
        "string",
        "null"
      ]
    },
    "id": {
      "description": "Installed provider package to load.",
      "type": "string"
    },
    "machine_id": {
      "description": "Enrolled machine authorized for private diagnostics.",
      "type": "string"
    },
    "offer": {
      "allOf": [
        {
          "$ref": "#/definitions/HarnessOffer"
        }
      ],
      "description": "Operator-approved candidate and public authentication information."
    },
    "space_id": {
      "description": "Accessible space used to acquire a disposable lease.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "machine_id",
    "offer",
    "space_id"
  ],
  "title": "HarnessStage",
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
