---
title: "plugins.enable"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Enable a trusted plugin

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "plugins.enable",
  "description": "Enable a trusted plugin",
  "mutates": true
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
    "ExtensionPoint": {
      "description": "Versioned host contribution slots in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.",
      "oneOf": [
        {
          "description": "Contribute to the detail view of an authorized note.",
          "enum": [
            "notes.detail"
          ],
          "type": "string"
        }
      ]
    },
    "False": {
      "const": false,
      "type": "boolean"
    },
    "GenUiAction": {
      "additionalProperties": false,
      "description": "Button operation call in `plugins.preview` and `notes-insights.summarize` views or plugin manifests; dispatch still checks current authority.",
      "properties": {
        "input": {
          "additionalProperties": true,
          "description": "Operation-specific JSON object; must pass the selected operation schema before dispatch.",
          "type": "object"
        },
        "operation": {
          "description": "Versioned operation identifier; operation envelopes and actions require 1..128 UTF-8 bytes.",
          "type": "string"
        }
      },
      "required": [
        "input",
        "operation"
      ],
      "type": "object"
    },
    "GenUiNode": {
      "description": "Portable declarative nodes in plugin previews and `notes-insights.summarize` results. Validation bounds depth to 8, nodes to 128 and enforces unique IDs.",
      "oneOf": [
        {
          "additionalProperties": false,
          "description": "Ordered container of child nodes.",
          "properties": {
            "children": {
              "description": "Ordered child nodes; the complete tree must remain within depth 8 and 128 nodes.",
              "items": {
                "$ref": "#/definitions/GenUiNode"
              },
              "type": "array"
            },
            "id": {
              "description": "Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.",
              "type": "string"
            },
            "type": {
              "enum": [
                "stack"
              ],
              "type": "string"
            }
          },
          "required": [
            "children",
            "id",
            "type"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Literal non-interactive text.",
          "properties": {
            "id": {
              "description": "Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.",
              "type": "string"
            },
            "text": {
              "description": "Literal display text, limited to 8 KiB of UTF-8.",
              "type": "string"
            },
            "type": {
              "enum": [
                "text"
              ],
              "type": "string"
            }
          },
          "required": [
            "id",
            "text",
            "type"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Labeled action dispatched through the operation boundary.",
          "properties": {
            "action": {
              "allOf": [
                {
                  "$ref": "#/definitions/GenUiAction"
                }
              ],
              "description": "Declared operation to dispatch on activation; no authority is embedded in the action."
            },
            "id": {
              "description": "Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.",
              "type": "string"
            },
            "label": {
              "description": "Accessible display label, 1..256 Unicode scalars.",
              "type": "string"
            },
            "type": {
              "enum": [
                "button"
              ],
              "type": "string"
            }
          },
          "required": [
            "action",
            "id",
            "label",
            "type"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Labeled statistic rendered as text.",
          "properties": {
            "id": {
              "description": "Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.",
              "type": "string"
            },
            "label": {
              "description": "Accessible display label, 1..256 Unicode scalars.",
              "type": "string"
            },
            "type": {
              "enum": [
                "stat"
              ],
              "type": "string"
            },
            "value": {
              "description": "Display-formatted statistic value; rendering must treat it as text.",
              "type": "string"
            }
          },
          "required": [
            "id",
            "label",
            "type",
            "value"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Link to a stable resource resolved through current access checks.",
          "properties": {
            "id": {
              "description": "Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.",
              "type": "string"
            },
            "label": {
              "description": "Accessible display label, 1..256 Unicode scalars.",
              "type": "string"
            },
            "resource_id": {
              "description": "Stable UUID resource target; resolving it requires access to its space.",
              "type": "string"
            },
            "space_id": {
              "description": "Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.",
              "type": "string"
            },
            "type": {
              "enum": [
                "resource"
              ],
              "type": "string"
            }
          },
          "required": [
            "id",
            "label",
            "resource_id",
            "space_id",
            "type"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Host-owned document editor bound to an authorized resource.",
          "properties": {
            "id": {
              "description": "Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.",
              "type": "string"
            },
            "resource_id": {
              "description": "Stable UUID resource target; resolving it requires access to its space.",
              "type": "string"
            },
            "space_id": {
              "description": "Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.",
              "type": "string"
            },
            "type": {
              "enum": [
                "document-editor"
              ],
              "type": "string"
            }
          },
          "required": [
            "id",
            "resource_id",
            "space_id",
            "type"
          ],
          "type": "object"
        }
      ]
    },
    "GenUiView": {
      "additionalProperties": false,
      "description": "Versioned portable view in plugin manifests, previews and `notes-insights.summarize`. Validation limits JSON to 64 KiB and string values to 32 KiB.",
      "properties": {
        "genui_version": {
          "description": "Portable view contract version; exactly 1 in this preview.",
          "format": "uint32",
          "maximum": 1,
          "minimum": 1,
          "type": "integer"
        },
        "root": {
          "allOf": [
            {
              "$ref": "#/definitions/GenUiNode"
            }
          ],
          "description": "Root node of the bounded portable tree; IDs are unique across the tree."
        }
      },
      "required": [
        "genui_version",
        "root"
      ],
      "type": "object"
    },
    "PluginDependency": {
      "additionalProperties": false,
      "description": "Exact plugin dependency declaration in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.",
      "properties": {
        "id": {
          "description": "Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.",
          "type": "string"
        },
        "version": {
          "description": "Nonempty plugin version string; dependencies declare the exact version they require.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "version"
      ],
      "type": "object"
    },
    "PluginManifest": {
      "additionalProperties": false,
      "description": "Versioned trusted-plugin metadata exposed by `plugins.list`, `plugins.enable` and `plugins.disable`. Declarations do not themselves grant authority.",
      "properties": {
        "api_version": {
          "description": "Supported plugin contract version; exactly 1 in this preview.",
          "format": "uint32",
          "maximum": 1,
          "minimum": 1,
          "type": "integer"
        },
        "capabilities": {
          "description": "Declared capability names; actual authority remains host-controlled.",
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "dependencies": {
          "description": "Required plugin IDs and exact versions; no ambient access is inherited from a dependency.",
          "items": {
            "$ref": "#/definitions/PluginDependency"
          },
          "type": "array"
        },
        "description": {
          "description": "Human-readable description for discovery; carries no execution authority.",
          "type": "string"
        },
        "id": {
          "description": "Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.",
          "type": "string"
        },
        "name": {
          "description": "Nonempty plugin display name for discovery.",
          "type": "string"
        },
        "operations": {
          "description": "Declared/discovered operation contracts; dispatch still checks current permissions.",
          "items": {
            "$ref": "#/definitions/PluginOperation"
          },
          "type": "array"
        },
        "version": {
          "description": "Nonempty plugin version string; dependencies declare the exact version they require.",
          "type": "string"
        },
        "views": {
          "description": "Portable contributions registered for supported host extension points.",
          "items": {
            "$ref": "#/definitions/PluginView"
          },
          "type": "array"
        }
      },
      "required": [
        "api_version",
        "capabilities",
        "dependencies",
        "description",
        "id",
        "name",
        "operations",
        "version",
        "views"
      ],
      "type": "object"
    },
    "PluginOperation": {
      "additionalProperties": false,
      "description": "Read-only extension operation declaration in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.",
      "properties": {
        "description": {
          "description": "Human-readable description for discovery; carries no execution authority.",
          "type": "string"
        },
        "id": {
          "description": "Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.",
          "type": "string"
        },
        "mutates": {
          "allOf": [
            {
              "$ref": "#/definitions/False"
            }
          ],
          "description": "Whether this operation changes durable state; extension declarations are constrained to false."
        }
      },
      "required": [
        "description",
        "id",
        "mutates"
      ],
      "type": "object"
    },
    "PluginState": {
      "additionalProperties": false,
      "description": "Space-scoped activation snapshot returned by `plugins.list`, `plugins.enable` and `plugins.disable`.",
      "properties": {
        "enabled": {
          "description": "Whether this plugin is currently active in the selected space.",
          "type": "boolean"
        },
        "generation": {
          "description": "Activation generation used to reject results from superseded plugin state.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "manifest": {
          "allOf": [
            {
              "$ref": "#/definitions/PluginManifest"
            }
          ],
          "description": "Registered trusted-plugin metadata; enabling it does not replace host enforcement."
        }
      },
      "required": [
        "enabled",
        "generation",
        "manifest"
      ],
      "type": "object"
    },
    "PluginView": {
      "additionalProperties": false,
      "description": "Portable view contribution in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.",
      "properties": {
        "extension_point": {
          "allOf": [
            {
              "$ref": "#/definitions/ExtensionPoint"
            }
          ],
          "description": "Host slot at which this contribution may be mounted."
        },
        "id": {
          "description": "Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.",
          "type": "string"
        },
        "view": {
          "allOf": [
            {
              "$ref": "#/definitions/GenUiView"
            }
          ],
          "description": "Validated portable presentation with bounded node, depth, text and serialized sizes."
        }
      },
      "required": [
        "extension_point",
        "id",
        "view"
      ],
      "type": "object"
    }
  },
  "description": "Result for `plugins.enable` and `plugins.disable` after the state transition.",
  "properties": {
    "plugin": {
      "allOf": [
        {
          "$ref": "#/definitions/PluginState"
        }
      ],
      "description": "Plugin state after the requested activation transition."
    }
  },
  "required": [
    "plugin"
  ],
  "title": "PluginResult",
  "type": "object"
}
```
