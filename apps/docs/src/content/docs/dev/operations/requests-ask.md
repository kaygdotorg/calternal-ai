---
title: "requests.ask"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Ask the person a question from the current turn

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "requests.ask",
  "description": "Ask the person a question from the current turn",
  "mutates": true
}
```

## Input

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
    }
  },
  "description": "Raise a question from the caller's current thread.",
  "properties": {
    "form": {
      "allOf": [
        {
          "$ref": "#/definitions/GenUiView"
        }
      ],
      "description": "Form capable of asking several questions at once."
    }
  },
  "required": [
    "form"
  ],
  "title": "RequestAskInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Result of resolving a private request.",
  "properties": {
    "id": {
      "description": "Resolved request UUID.",
      "type": "string"
    },
    "status": {
      "description": "Answered, approved or denied.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "status"
  ],
  "title": "RequestResolution",
  "type": "object"
}
```
