---
title: "trash.empty.preview"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Count notes and retained version bytes before emptying Trash

Capability: content.purge.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "content.purge",
  "id": "trash.empty.preview",
  "description": "Count notes and retained version bytes before emptying Trash",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Selects the Trash of one space for an irreversible-action preview.",
  "properties": {
    "space_id": {
      "description": "UUID of the owning space.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "TrashScope",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Confirmation for exactly the current set of trashed revisions.",
  "properties": {
    "bytes": {
      "description": "Logical UTF-8 bytes in all retained versions of the selected notes.",
      "format": "uint64",
      "minimum": 0,
      "type": "integer"
    },
    "confirmation": {
      "description": "Opaque set fingerprint required when starting the job.",
      "type": "string"
    },
    "count": {
      "description": "Number of selected notes.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "bytes",
    "confirmation",
    "count"
  ],
  "title": "TrashEmptyPreview",
  "type": "object"
}
```
