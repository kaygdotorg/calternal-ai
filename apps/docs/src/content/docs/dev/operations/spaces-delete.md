---
title: "spaces.delete"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Delete directory records; content cleanup is a later step

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "spaces.delete",
  "description": "Delete directory records; content cleanup is a later step",
  "mutates": true
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
      "description": "Stable space UUID; development inputs temporarily accept seeded aliases.",
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
  "description": "A completed membership mutation or directory-record deletion.",
  "title": "SpaceChangedResult",
  "type": "object"
}
```
