---
title: "fonts.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List shared and caller-owned fonts

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "fonts.list",
  "description": "List shared and caller-owned fonts",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Page of installed families and the authenticated account's own uploads.",
  "properties": {
    "cursor": {
      "description": "Opaque next cursor from a preceding page.",
      "type": [
        "string",
        "null"
      ]
    },
    "limit": {
      "description": "Page size from 1 to 100. Defaults to 30.",
      "format": "uint16",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    }
  },
  "title": "FontListInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "FontRole": {
      "description": "Each independent typography role in the shell.",
      "oneOf": [
        {
          "description": "Interface labels and controls.",
          "enum": [
            "ui"
          ],
          "type": "string"
        },
        {
          "description": "Conversation text.",
          "enum": [
            "chat"
          ],
          "type": "string"
        },
        {
          "description": "Code.",
          "enum": [
            "code"
          ],
          "type": "string"
        },
        {
          "description": "Terminal.",
          "enum": [
            "terminal"
          ],
          "type": "string"
        },
        {
          "description": "Wordmark.",
          "enum": [
            "brand"
          ],
          "type": "string"
        }
      ]
    },
    "FontSource": {
      "description": "Available catalogue and local font sources.",
      "oneOf": [
        {
          "description": "Google Fonts.",
          "enum": [
            "google"
          ],
          "type": "string"
        },
        {
          "description": "Fontsource.",
          "enum": [
            "fontsource"
          ],
          "type": "string"
        },
        {
          "description": "Fontshare.",
          "enum": [
            "fontshare"
          ],
          "type": "string"
        },
        {
          "description": "An account's own file.",
          "enum": [
            "upload"
          ],
          "type": "string"
        },
        {
          "description": "The device's system typeface.",
          "enum": [
            "system"
          ],
          "type": "string"
        }
      ]
    },
    "FontStoredFamily": {
      "description": "Metadata for one locally usable family.",
      "properties": {
        "bytes": {
          "description": "Byte count of stored files.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "family": {
          "description": "Display family name.",
          "type": "string"
        },
        "id": {
          "description": "Stable library ID.",
          "type": "string"
        },
        "source": {
          "allOf": [
            {
              "$ref": "#/definitions/FontSource"
            }
          ],
          "description": "Provider or Upload."
        },
        "used_by": {
          "description": "Roles in which the authenticated account uses this family.",
          "items": {
            "$ref": "#/definitions/FontRole"
          },
          "type": "array"
        }
      },
      "required": [
        "bytes",
        "family",
        "id",
        "source",
        "used_by"
      ],
      "type": "object"
    }
  },
  "description": "The user's visible local font storage.",
  "properties": {
    "families": {
      "description": "Current page of installed or owned families.",
      "items": {
        "$ref": "#/definitions/FontStoredFamily"
      },
      "type": "array"
    },
    "next_cursor": {
      "description": "Next cursor or null.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "families"
  ],
  "title": "FontListResult",
  "type": "object"
}
```
