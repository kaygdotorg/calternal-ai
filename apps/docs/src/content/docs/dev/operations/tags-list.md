---
title: "tags.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List tags and their current note counts

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "tags.list",
  "description": "List tags and their current note counts",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for members, leave and directory-record deletion.",
  "properties": {
    "space_id": {
      "description": "Space UUID; names are never accepted in its place.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceInput",
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
