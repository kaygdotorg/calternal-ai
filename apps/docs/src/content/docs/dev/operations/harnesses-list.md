---
title: "harnesses.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List installed harnesses and their tested updates

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "harnesses.list",
  "description": "List installed harnesses and their tested updates",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Account-private list input. No caller-supplied account is accepted.",
  "title": "AccountInputList",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
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
    },
    "HarnessStatus": {
      "description": "One installation-level runtime and its available changes.",
      "properties": {
        "active": {
          "anyOf": [
            {
              "$ref": "#/definitions/HarnessPin"
            },
            {
              "type": "null"
            }
          ],
          "description": "Current immutable runtime pin, if one was activated."
        },
        "bundled": {
          "description": "Whether the package has a bundled legacy runtime.",
          "type": "boolean"
        },
        "id": {
          "description": "Installed provider package identifier.",
          "type": "string"
        },
        "name": {
          "description": "Human-readable provider name.",
          "type": "string"
        },
        "previous": {
          "anyOf": [
            {
              "$ref": "#/definitions/HarnessPin"
            },
            {
              "type": "null"
            }
          ],
          "description": "Prior pin offered for rollback."
        },
        "tested": {
          "anyOf": [
            {
              "$ref": "#/definitions/HarnessPin"
            },
            {
              "type": "null"
            }
          ],
          "description": "Candidate offered for activation after a successful smoke test."
        }
      },
      "required": [
        "bundled",
        "id",
        "name"
      ],
      "type": "object"
    }
  },
  "description": "Catalog of installed harnesses visible to the installation administrator.",
  "properties": {
    "harnesses": {
      "description": "Installed provider packages and their runtime status.",
      "items": {
        "$ref": "#/definitions/HarnessStatus"
      },
      "type": "array"
    }
  },
  "required": [
    "harnesses"
  ],
  "title": "HarnessList",
  "type": "object"
}
```
