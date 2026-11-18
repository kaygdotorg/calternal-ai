---
title: "plugins.manifests"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List palette contributions from enabled plugins.

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "space",
  "id": "plugins.manifests",
  "description": "List palette contributions from enabled plugins.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for `plugins.list`; state is resolved within the authorized space.",
  "properties": {
    "space_id": {
      "description": "Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.",
      "maxLength": 80,
      "minLength": 1,
      "pattern": "^[a-zA-Z0-9_.:-]+$",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "PluginsListInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "PaletteCommand": {
      "additionalProperties": false,
      "description": "One palette command, in the web palette's command shape.",
      "properties": {
        "go": {
          "description": "Navigation address, if this command navigates.",
          "type": [
            "string",
            "null"
          ]
        },
        "group": {
          "description": "Display group.",
          "type": "string"
        },
        "id": {
          "description": "Stable, globally unique command identifier.",
          "type": "string"
        },
        "label": {
          "description": "Searchable command label.",
          "type": "string"
        },
        "op": {
          "description": "Named host action, if the command invokes one.",
          "type": [
            "string",
            "null"
          ]
        },
        "words": {
          "description": "Extra phrases that match this command.",
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "group",
        "id",
        "label"
      ],
      "type": "object"
    },
    "PaletteManifest": {
      "description": "A registered palette contribution in the web palette's manifest shape.",
      "properties": {
        "commands": {
          "description": "Commands with their extra search phrases.",
          "items": {
            "$ref": "#/definitions/PaletteCommand"
          },
          "type": "array"
        },
        "plugin": {
          "description": "Owning plugin identifier.",
          "type": "string"
        },
        "settings_sections": {
          "description": "Settings sections provided by this plugin.",
          "items": {
            "$ref": "#/definitions/PaletteSettingsSection"
          },
          "type": "array"
        },
        "version": {
          "description": "Exact package version.",
          "type": "string"
        }
      },
      "required": [
        "commands",
        "plugin",
        "settings_sections",
        "version"
      ],
      "type": "object"
    },
    "PaletteSettingsSection": {
      "additionalProperties": false,
      "description": "A discoverable Settings section.",
      "properties": {
        "go": {
          "description": "Settings address.",
          "type": "string"
        },
        "id": {
          "description": "Stable section identifier.",
          "type": "string"
        },
        "label": {
          "description": "Section label.",
          "type": "string"
        }
      },
      "required": [
        "go",
        "id",
        "label"
      ],
      "type": "object"
    }
  },
  "description": "Palette declarations visible in the selected space.",
  "properties": {
    "manifests": {
      "description": "Enabled plugins with palette contributions.",
      "items": {
        "$ref": "#/definitions/PaletteManifest"
      },
      "type": "array"
    }
  },
  "required": [
    "manifests"
  ],
  "title": "PaletteResult",
  "type": "object"
}
```
