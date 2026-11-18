---
title: "fonts.status"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Check a family's local availability

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "fonts.status",
  "description": "Check a family's local availability",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Opaque family identifier, resolved against the caller's permitted library.",
  "properties": {
    "id": {
      "description": "Family ID returned by the catalogue, list or upload reply.",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "FontIdInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "FontInstallState": {
      "description": "Availability of one family at this installation.",
      "oneOf": [
        {
          "description": "No published family or active fill.",
          "enum": [
            "missing"
          ],
          "type": "string"
        },
        {
          "description": "A shared fill is in progress.",
          "enum": [
            "installing"
          ],
          "type": "string"
        },
        {
          "description": "The requested local family is available.",
          "enum": [
            "ready"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "Local install state and the installation's download policy.",
  "properties": {
    "downloads_enabled": {
      "description": "Whether remote requests are enabled.",
      "type": "boolean"
    },
    "state": {
      "allOf": [
        {
          "$ref": "#/definitions/FontInstallState"
        }
      ],
      "description": "Family state."
    }
  },
  "required": [
    "downloads_enabled",
    "state"
  ],
  "title": "FontStatusResult",
  "type": "object"
}
```
