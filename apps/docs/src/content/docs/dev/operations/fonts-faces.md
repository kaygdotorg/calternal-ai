---
title: "fonts.faces"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Generate same-origin face rules

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "fonts.faces",
  "description": "Generate same-origin face rules",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Generate CSS for the account's roles, or inspect one family for a preview.",
  "properties": {
    "id": {
      "description": "Optional ID of a local family to preview.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "FontFacesInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Local CSS with typeface aliases for clients' font stacks.",
  "properties": {
    "css": {
      "description": "Generated rules with same-origin URLs.",
      "type": "string"
    },
    "families": {
      "additionalProperties": {
        "type": "string"
      },
      "description": "Library ID to generated CSS family alias.",
      "type": "object"
    }
  },
  "required": [
    "css",
    "families"
  ],
  "title": "FontFacesResult",
  "type": "object"
}
```
