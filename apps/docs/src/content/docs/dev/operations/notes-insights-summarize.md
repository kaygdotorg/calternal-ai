---
title: "notes-insights.summarize"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Compute word, character and line statistics

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "notes-insights.summarize",
  "description": "Compute word, character and line statistics",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for `notes.get` and `notes-insights.summarize`; a resource ID alone grants no access.",
  "properties": {
    "id": {
      "description": "Stable globally unique note UUID; the server resolves its owning space.",
      "type": "string"
    },
    "space_id": {
      "description": "Optional compatibility space hint; omission derives the owning space from `id`.",
      "maxLength": 80,
      "minLength": 1,
      "pattern": "^[a-zA-Z0-9_.:-]+$",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "NotesGetInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "GenUiView": {
      "additionalProperties": false,
      "description": "A generated-UI view: a program in the GenUI language (`crates/genui`, `packages/genui`) that each client compiles and evaluates with its own native components. The host compiles it before delivery and refuses a view with any diagnostic; actions in it describe operations, which dispatch checks again.",
      "properties": {
        "genui_version": {
          "description": "Generated-UI language version; exactly 2 (version 1 was the retired JSON tree).",
          "format": "uint32",
          "maximum": 2,
          "minimum": 2,
          "type": "integer"
        },
        "source": {
          "description": "Program source, at most 64 KiB of UTF-8.",
          "maxLength": 65536,
          "type": "string"
        }
      },
      "required": [
        "genui_version",
        "source"
      ],
      "type": "object"
    },
    "Stats": {
      "additionalProperties": false,
      "description": "Read-only Markdown statistics returned by `notes-insights.summarize`.",
      "properties": {
        "characters": {
          "description": "Count of Unicode scalar values, not UTF-8 bytes or grapheme clusters.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "lines": {
          "description": "Logical line count; CRLF counts as one break, a trailing break adds no empty line, and empty text has zero lines.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "words": {
          "description": "Count of Unicode letter/number word runs, allowing internal straight or curly apostrophes.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "characters",
        "lines",
        "words"
      ],
      "type": "object"
    }
  },
  "description": "Result for `notes-insights.summarize`, combining statistics with a portable presentation.",
  "properties": {
    "stats": {
      "allOf": [
        {
          "$ref": "#/definitions/Stats"
        }
      ],
      "description": "Counts computed from the supplied Markdown snapshot without modifying it."
    },
    "view": {
      "allOf": [
        {
          "$ref": "#/definitions/GenUiView"
        }
      ],
      "description": "Generated-UI view, compiled by the host before delivery."
    }
  },
  "required": [
    "stats",
    "view"
  ],
  "title": "SummarizeResult",
  "type": "object"
}
```
