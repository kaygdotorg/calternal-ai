---
title: "tags.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List a page of up to 100 tags and their current note counts

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "id": "tags.list",
  "description": "List a page of up to 100 tags and their current note counts",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A continuation scoped to one space.",
  "properties": {
    "cursor": {
      "description": "The previous page's cursor for this space and operation.",
      "maxLength": 524288,
      "minLength": 1,
      "type": "string"
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceDirectoryPageInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "TagCount": {
      "additionalProperties": false,
      "description": "A named tag and its current note count in one space.",
      "properties": {
        "count": {
          "description": "Number of live notes with this tag or one nested underneath it.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "name": {
          "description": "Tag name without a leading hash.",
          "type": "string"
        }
      },
      "required": [
        "count",
        "name"
      ],
      "type": "object"
    }
  },
  "description": "Bounded tag lookup result.",
  "properties": {
    "next_cursor": {
      "description": "Opaque continuation for the same space and prefix, omitted at the end.",
      "type": [
        "string",
        "null"
      ]
    },
    "tags": {
      "description": "Up to 100 tag names and counts, in literal name order.",
      "items": {
        "$ref": "#/definitions/TagCount"
      },
      "type": "array"
    }
  },
  "required": [
    "tags"
  ],
  "title": "TagsResult",
  "type": "object"
}
```
