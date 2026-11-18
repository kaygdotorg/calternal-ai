---
title: "fonts.catalogue"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Search the bounded shared catalogue

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "fonts.catalogue",
  "description": "Search the bounded shared catalogue",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
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
    }
  },
  "description": "Fetch a page of remote families using local search.",
  "properties": {
    "cursor": {
      "description": "Opaque next cursor from the same filters and cached snapshot.",
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
    },
    "mono": {
      "description": "Optionally filter by monospace classification.",
      "type": [
        "boolean",
        "null"
      ]
    },
    "query": {
      "description": "Case-insensitive substring, at most 120 characters.",
      "type": [
        "string",
        "null"
      ]
    },
    "source": {
      "anyOf": [
        {
          "$ref": "#/definitions/FontSource"
        },
        {
          "type": "null"
        }
      ],
      "description": "Optionally filter by a remote provider."
    }
  },
  "title": "FontCatalogueInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "FontCatalogueFamily": {
      "description": "A merged catalogue row.",
      "properties": {
        "category": {
          "description": "Source category normalized by the server.",
          "type": "string"
        },
        "family": {
          "description": "Display family name.",
          "type": "string"
        },
        "id": {
          "description": "Stable ID for installation.",
          "type": "string"
        },
        "mono": {
          "description": "Whether the provider classifies this as monospaced.",
          "type": "boolean"
        },
        "source": {
          "allOf": [
            {
              "$ref": "#/definitions/FontSource"
            }
          ],
          "description": "Provider."
        }
      },
      "required": [
        "category",
        "family",
        "id",
        "mono",
        "source"
      ],
      "type": "object"
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
    }
  },
  "description": "A page of remote catalogue families.",
  "properties": {
    "families": {
      "description": "Matching families in deterministic order.",
      "items": {
        "$ref": "#/definitions/FontCatalogueFamily"
      },
      "type": "array"
    },
    "next_cursor": {
      "description": "Cursor for the next page, or null at the end.",
      "type": [
        "string",
        "null"
      ]
    },
    "partial": {
      "description": "Whether a source is unreachable, stale or disabled.",
      "type": "boolean"
    }
  },
  "required": [
    "families",
    "partial"
  ],
  "title": "FontCatalogueResult",
  "type": "object"
}
```
