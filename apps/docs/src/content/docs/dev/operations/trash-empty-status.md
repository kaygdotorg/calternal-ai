---
title: "trash.empty.status"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read progress of an Empty Trash job

Capability: space.settings.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.settings",
  "id": "trash.empty.status",
  "description": "Read progress of an Empty Trash job",
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
  "description": "Durable progress of one irreversible Empty Trash job.",
  "properties": {
    "bytes": {
      "description": "Logical version bytes selected at confirmation.",
      "format": "uint64",
      "minimum": 0,
      "type": "integer"
    },
    "count": {
      "description": "Number of notes selected at confirmation.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "job_id": {
      "description": "Stable job UUID.",
      "type": "string"
    },
    "purged": {
      "description": "Number of selected revisions permanently removed by this job.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "skipped": {
      "description": "Number of selected revisions restored, replaced, blocked or already removed.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "state": {
      "description": "`pending` while work remains, `done` after all selected revisions are examined.",
      "type": "string"
    }
  },
  "required": [
    "bytes",
    "count",
    "job_id",
    "purged",
    "skipped",
    "state"
  ],
  "title": "TrashJob",
  "type": "object"
}
```
