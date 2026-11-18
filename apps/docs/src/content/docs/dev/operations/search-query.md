---
title: "search.query"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Search titles and bodies across readable spaces.

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "account",
  "id": "search.query",
  "description": "Search titles and bodies across readable spaces.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Search visible items by title and body. An omitted space searches all readable spaces.",
  "properties": {
    "cursor": {
      "description": "Opaque continuation from this operation and filter set.",
      "type": [
        "string",
        "null"
      ]
    },
    "kinds": {
      "description": "Kind filter. An empty list selects all opted-in kinds.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "limit": {
      "description": "Maximum rows to return, from 1 to 50. Defaults to 20.",
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "query": {
      "description": "Text typed into the palette.",
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
    "query"
  ],
  "title": "SearchInput",
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
        "message_at": {
          "description": "Saved RFC 3339 message time, with its original offset.",
          "type": [
            "string",
            "null"
          ]
        },
        "message_id": {
          "description": "Stable record UUID on a message hit.",
          "type": [
            "string",
            "null"
          ]
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
