---
title: "plugins.preview"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Preview a trusted plugin using fixture data

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "plugins.preview",
  "description": "Preview a trusted plugin using fixture data",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for `plugins.enable`, `plugins.disable` and `plugins.preview`.",
  "properties": {
    "plugin_id": {
      "description": "Registered trusted-plugin identifier, not a package path or source of authority.",
      "type": "string"
    },
    "space_id": {
      "description": "Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.",
      "maxLength": 80,
      "minLength": 1,
      "pattern": "^[a-zA-Z0-9_.:-]+$",
      "type": "string"
    }
  },
  "required": [
    "plugin_id",
    "space_id"
  ],
  "title": "PluginInput",
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
    "True": {
      "const": true,
      "type": "boolean"
    }
  },
  "description": "Result for `plugins.preview`, computed from fixture data rather than private note content.",
  "properties": {
    "preview": {
      "allOf": [
        {
          "$ref": "#/definitions/True"
        }
      ],
      "description": "Always true: this view was prepared with disposable fixture data."
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
    "preview",
    "view"
  ],
  "title": "PluginPreviewResult",
  "type": "object"
}
```
