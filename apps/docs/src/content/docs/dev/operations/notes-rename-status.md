---
title: "notes.rename.status"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read progress of a Notes rename or its inverse

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "id": "notes.rename.status",
  "description": "Read progress of a Notes rename or its inverse",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Select one previously admitted job in its owning space.",
  "properties": {
    "job_id": {
      "description": "Job UUID returned on admission.",
      "type": "string"
    },
    "space_id": {
      "description": "UUID of the owning space.",
      "type": "string"
    }
  },
  "required": [
    "job_id",
    "space_id"
  ],
  "title": "NoteJobInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Progress of an authorized Notes rename or its inverse.",
  "properties": {
    "job_id": {
      "description": "Job UUID returned by the rename.",
      "type": "string"
    },
    "note_id": {
      "description": "Stable identity of the renamed note.",
      "type": "string"
    },
    "remaining": {
      "description": "Number of source files still to inspect in the current direction.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "skipped": {
      "description": "Number of conflicts skipped in the current direction.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "state": {
      "description": "`pending`, `done`, `undoing`, `undone`, or `blocked`.",
      "type": "string"
    },
    "total": {
      "description": "Original number of candidate linking notes.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "job_id",
    "note_id",
    "remaining",
    "skipped",
    "state",
    "total"
  ],
  "title": "NoteRenameJob",
  "type": "object"
}
```
