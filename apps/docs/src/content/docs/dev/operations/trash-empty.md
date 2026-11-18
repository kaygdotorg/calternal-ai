---
title: "trash.empty"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Start a resumable job to permanently purge the confirmed Trash selection

Capability: content.purge.

## Declaration

```json
{
  "risk": "irreversible",
  "required_capability": "content.purge",
  "id": "trash.empty",
  "description": "Start a resumable job to permanently purge the confirmed Trash selection",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Confirm the irreversible deletion of a previewed Trash selection.",
  "properties": {
    "confirmation": {
      "description": "Confirmation from a fresh preview for the same space.",
      "type": "string"
    },
    "space_id": {
      "description": "UUID of the owning space.",
      "type": "string"
    }
  },
  "required": [
    "confirmation",
    "space_id"
  ],
  "title": "TrashEmptyInput",
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
