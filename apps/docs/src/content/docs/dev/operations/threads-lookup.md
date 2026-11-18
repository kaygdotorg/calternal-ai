---
title: "threads.lookup"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Find threads by title prefix across readable spaces.

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "account",
  "id": "threads.lookup",
  "description": "Find threads by title prefix across readable spaces.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Picker filter for cross-space item and thread lookups.",
  "properties": {
    "cursor": {
      "description": "Opaque continuation from the same lookup and filters.",
      "type": [
        "string",
        "null"
      ]
    },
    "limit": {
      "description": "Maximum rows, from 1 to 50. Defaults to 20.",
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "prefix": {
      "description": "Literal title prefix.",
      "type": "string"
    },
    "space_id": {
      "description": "Optional space UUID.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "prefix"
  ],
  "title": "LookupInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "SearchHit": {
      "description": "One indexed item in its visible space.",
      "properties": {
        "address": {
          "description": "Stable item address.",
          "type": "string"
        },
        "excerpt": {
          "description": "Short title or body excerpt with structured highlights.",
          "items": {
            "$ref": "#/definitions/SearchSegment"
          },
          "type": "array"
        },
        "kind": {
          "description": "Manifest kind identifier.",
          "type": "string"
        },
        "space_id": {
          "description": "Owning space UUID.",
          "type": "string"
        },
        "title": {
          "description": "Display title.",
          "type": "string"
        }
      },
      "required": [
        "address",
        "excerpt",
        "kind",
        "space_id",
        "title"
      ],
      "type": "object"
    },
    "SearchSegment": {
      "description": "One plain-text excerpt segment. Render its text literally.",
      "properties": {
        "matched": {
          "description": "Whether the segment matches the search.",
          "type": "boolean"
        },
        "text": {
          "description": "Literal display text.",
          "type": "string"
        }
      },
      "required": [
        "matched",
        "text"
      ],
      "type": "object"
    }
  },
  "description": "Visible results in deterministic order.",
  "properties": {
    "hits": {
      "description": "The current page.",
      "items": {
        "$ref": "#/definitions/SearchHit"
      },
      "type": "array"
    },
    "next_cursor": {
      "description": "Opaque continuation, omitted at the end.",
      "type": [
        "string",
        "null"
      ]
    },
    "partial": {
      "description": "A readable selected index is catching up, rebuilding, or has reached its coverage limit.",
      "type": "boolean"
    }
  },
  "required": [
    "hits",
    "partial"
  ],
  "title": "SearchResult",
  "type": "object"
}
```
