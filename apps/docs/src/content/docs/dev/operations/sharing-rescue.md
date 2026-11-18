---
title: "sharing.rescue"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Save the caller's blocked offline edits as a private note

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "sharing.rescue",
  "description": "Save the caller's blocked offline edits as a private note",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Locally authored edits the server could no longer save to a shared note.",
  "properties": {
    "changed_blocks": {
      "description": "Only the caller's locally changed Markdown blocks, in document order.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "title": {
      "description": "The former note title, from the caller's local copy.",
      "type": "string"
    }
  },
  "required": [
    "changed_blocks",
    "title"
  ],
  "title": "ShareRescueInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": true,
  "title": "Map_of_AnyValue",
  "type": "object"
}
```
