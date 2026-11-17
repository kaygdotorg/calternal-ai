---
title: "space.instructions"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read or replace standing space instructions

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "irreversible",
  "id": "space.instructions",
  "description": "Read or replace standing space instructions",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Standing instructions for one space. Omitting Markdown reads the file.",
  "properties": {
    "markdown": {
      "description": "Replacement Markdown, limited to 16 KiB.",
      "type": [
        "string",
        "null"
      ]
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceInstructionsInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "The bounded standing instructions document.",
  "properties": {
    "markdown": {
      "description": "Markdown text. An absent file is represented by empty text.",
      "type": "string"
    }
  },
  "required": [
    "markdown"
  ],
  "title": "SpaceInstructionsResult",
  "type": "object"
}
```
