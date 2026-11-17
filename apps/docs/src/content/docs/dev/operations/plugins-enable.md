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
  "risk": "widens_access",
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
    "HostCallKind": {
      "description": "The only broker methods a worker may request.",
      "oneOf": [
        {
          "description": "Read an authorized item.",
          "enum": [
            "read_item"
          ],
          "type": "string"
        },
        {
          "description": "Write an authorized item.",
          "enum": [
            "write_item"
          ],
          "type": "string"
        },
        {
          "description": "List an authorized collection.",
          "enum": [
            "list_collection"
          ],
          "type": "string"
        },
        {
          "description": "Emit an authorized event.",
          "enum": [
            "emit_event"
          ],
          "type": "string"
        },
        {
          "description": "Request durable job admission.",
          "enum": [
            "start_job"
          ],
          "type": "string"
        },
        {
          "description": "Durably record a running job's checkpoint, from its run invocation.",
          "enum": [
            "checkpoint_job"
          ],
          "type": "string"
        },
        {
          "description": "Request a scoped opaque handle, never secret bytes.",
          "enum": [
            "request_secret_handle"
          ],
          "type": "string"
        }
      ]
    },
    "ManifestAttention": {
      "additionalProperties": false,
      "description": "Authorized attention candidates; the shell selects at most three.",
      "properties": {
        "id": {
          "description": "Stable attention source identifier.",
          "type": "string"
        },
        "operation": {
          "description": "Declared operation providing candidates.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "operation"
      ],
      "type": "object"
    },
    "ManifestCapability": {
      "description": "Closed host vocabulary, matching protected host capability names.",
      "oneOf": [
        {
          "description": "Read authorized space content.",
          "enum": [
            "space.read"
          ],
          "type": "string"
        },
        {
          "description": "Write authorized space content.",
          "enum": [
            "content.write"
          ],
          "type": "string"
        },
        {
          "description": "Use installed plugins.",
          "enum": [
            "plugins.use"
          ],
          "type": "string"
        },
        {
          "description": "Manage shared plugin installations.",
          "enum": [
            "plugins.manage"
          ],
          "type": "string"
        },
        {
          "description": "Invite space members.",
          "enum": [
            "members.invite"
          ],
          "type": "string"
        },
        {
          "description": "Manage ordinary space members.",
          "enum": [
            "members.manage"
          ],
          "type": "string"
        },
        {
          "description": "Manage Managers; owner constraints still apply.",
          "enum": [
            "managers.manage"
          ],
          "type": "string"
        },
        {
          "description": "Change shared settings.",
          "enum": [
            "space.settings"
          ],
          "type": "string"
        },
        {
          "description": "Explicitly transfer ownership.",
          "enum": [
            "space.transfer"
          ],
          "type": "string"
        },
        {
          "description": "Delete a space.",
          "enum": [
            "space.delete"
          ],
          "type": "string"
        },
        {
          "description": "List the acting account memberships.",
          "enum": [
            "spaces.list"
          ],
          "type": "string"
        },
        {
          "description": "Create a space for the acting account.",
          "enum": [
            "spaces.create"
          ],
          "type": "string"
        }
      ]
    },
    "ManifestCollection": {
      "additionalProperties": false,
      "description": "Provisioning template; the host allocates a fresh Collection UUID per enable.",
      "properties": {
        "id": {
          "description": "Globally unique template identifier, never a Collection address.",
          "type": "string"
        },
        "kind": {
          "description": "Kind of items in this collection.",
          "type": "string"
        },
        "name": {
          "description": "Display name for provisioned collections.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "kind",
        "name"
      ],
      "type": "object"
    },
    "ManifestHook": {
      "additionalProperties": false,
      "description": "Space-scoped hook enabled by a Manager; never a global interceptor.",
      "properties": {
        "id": {
          "description": "Stable hook identifier.",
          "type": "string"
        },
        "mode": {
          "allOf": [
            {
              "$ref": "#/definitions/ManifestHookMode"
            }
          ],
          "description": "Watch or block only."
        },
        "operation": {
          "description": "Declared operation implementing the hook.",
          "type": "string"
        },
        "patterns": {
          "description": "Exact operation IDs or namespace patterns ending in .* .",
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "id",
        "mode",
        "operation",
        "patterns"
      ],
      "type": "object"
    },
    "ManifestHookMode": {
      "description": "Hooks observe or block; rewriting is unavailable.",
      "oneOf": [
        {
          "description": "Observe permitted matching operations.",
          "enum": [
            "watch"
          ],
          "type": "string"
        },
        {
          "description": "Return proceed, wait or reject under a host deadline.",
          "enum": [
            "block"
          ],
          "type": "string"
        }
      ]
    },
    "ManifestIdempotency": {
      "description": "Retry behavior declared by an operation.",
      "oneOf": [
        {
          "description": "Read with no durable effect.",
          "enum": [
            "read_only"
          ],
          "type": "string"
        },
        {
          "description": "Host receipt required before durable effects.",
          "enum": [
            "request_id"
          ],
          "type": "string"
        },
        {
          "description": "Automatic retry prohibited; reconcile uncertain outcomes.",
          "enum": [
            "never"
          ],
          "type": "string"
        }
      ]
    },
    "ManifestKind": {
      "additionalProperties": false,
      "description": "Kind identity remains independent of plugin identity.",
      "properties": {
        "claimed_text": {
          "description": "True selects userdata-ssd for claimed text; false selects userdata-hdd for other files.",
          "type": "boolean"
        },
        "extension": {
          "description": "Lowercase ASCII file extension without the dot (1 to 16 letters or digits). Claimed-text kinds are Markdown: absent or `md`. Other kinds must declare one; the host stores their bytes unparsed under it.",
          "type": [
            "string",
            "null"
          ]
        },
        "id": {
          "description": "Stable kind name used in item addresses.",
          "type": "string"
        },
        "schema": {
          "description": "JSON Schema for the kind content; no executable code."
        },
        "storage": {
          "allOf": [
            {
              "$ref": "#/definitions/ManifestPlacement"
            }
          ],
          "description": "Placement within the selected per-space userdata tree; never an appdata database path."
        }
      },
      "required": [
        "claimed_text",
        "id",
        "schema",
        "storage"
      ],
      "type": "object"
    },
    "ManifestOperation": {
      "additionalProperties": false,
      "description": "Public operation declaration consumed by host policy.",
      "properties": {
        "id": {
          "description": "Globally unique operation identifier.",
          "type": "string"
        },
        "idempotency": {
          "allOf": [
            {
              "$ref": "#/definitions/ManifestIdempotency"
            }
          ],
          "description": "Retry and host receipt contract."
        },
        "input_schema": {
          "description": "JSON Schema of operation input."
        },
        "mutates": {
          "description": "Whether this operation can produce durable effects.",
          "type": "boolean"
        },
        "output_schema": {
          "description": "JSON Schema of successful output."
        },
        "required_capability": {
          "allOf": [
            {
              "$ref": "#/definitions/ManifestCapability"
            }
          ],
          "description": "Required current host capability."
        },
        "risk": {
          "allOf": [
            {
              "$ref": "#/definitions/OperationRisk"
            }
          ],
          "description": "Required risk declaration; capability checks still apply in every mode."
        },
        "target": {
          "anyOf": [
            {
              "$ref": "#/definitions/ManifestTarget"
            },
            {
              "type": "null"
            }
          ],
          "description": "Item the operation targets; absent (or null) for a space-scoped operation, whose wire input names its space in `space_id`."
        }
      },
      "required": [
        "id",
        "idempotency",
        "input_schema",
        "mutates",
        "output_schema",
        "required_capability",
        "risk"
      ],
      "type": "object"
    },
    "ManifestPlacement": {
      "anyOf": [
        {
          "additionalProperties": false,
          "description": "Visible subtree claimed by one plugin, with at most eight safe components.",
          "properties": {
            "folder": {
              "description": "One safe visible top-level folder.",
              "type": "string"
            },
            "subdirectories": {
              "description": "Safe directory components below the folder; never a filesystem path.",
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "folder",
            "subdirectories"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "User-visible top-level folder; the host rejects cross-plugin folder claims.",
          "properties": {
            "folder": {
              "description": "One NFC folder component, with no leading dot or reserved name.",
              "type": "string"
            }
          },
          "required": [
            "folder"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Plugin-owned files ordinary people need not see.",
          "properties": {
            "hidden": {
              "description": "Must be true; placement is `.calternal/<plugin-id>/<kind>/` within userdata.",
              "type": "boolean"
            }
          },
          "required": [
            "hidden"
          ],
          "type": "object"
        }
      ],
      "description": "Placement inside a space userdata tree; hidden files belong to the declaring plugin."
    },
    "ManifestProvider": {
      "additionalProperties": false,
      "description": "Machine-side provider declaration; executable is installed in the sandbox image or account home, never downloaded or run by the server.",
      "properties": {
        "default_model": {
          "description": "Default installed-CLI model identifier, at most 128 UTF-8 bytes.",
          "type": "string"
        },
        "executable": {
          "description": "Basename resolved inside the sandbox's PATH; no shell, path or arguments.",
          "type": "string"
        },
        "protocol_version": {
          "description": "Version of the bounded stdio contract, currently one.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "default_model",
        "executable",
        "protocol_version"
      ],
      "type": "object"
    },
    "ManifestRegion": {
      "description": "Shell region populated by a component reference.",
      "oneOf": [
        {
          "description": "App sidebar body.",
          "enum": [
            "sidebar"
          ],
          "type": "string"
        },
        {
          "description": "Main card.",
          "enum": [
            "main"
          ],
          "type": "string"
        },
        {
          "description": "Panel in the right card.",
          "enum": [
            "right_card"
          ],
          "type": "string"
        }
      ]
    },
    "ManifestSchedule": {
      "additionalProperties": false,
      "description": "Schedule declaration; durable execution belongs to the host.",
      "properties": {
        "expression": {
          "description": "Schedule expression; activation awaits host scheduler validation.",
          "type": "string"
        },
        "id": {
          "description": "Stable schedule identifier.",
          "type": "string"
        },
        "operation": {
          "description": "Declared operation to invoke.",
          "type": "string"
        }
      },
      "required": [
        "expression",
        "id",
        "operation"
      ],
      "type": "object"
    },
    "ManifestSetting": {
      "additionalProperties": false,
      "description": "Typed space setting; secrets use separate handles.",
      "properties": {
        "default": {
          "description": "Default setting value."
        },
        "id": {
          "description": "Stable setting identifier.",
          "type": "string"
        },
        "value_type": {
          "allOf": [
            {
              "$ref": "#/definitions/ManifestSettingType"
            }
          ],
          "description": "Primitive type of the default and stored values."
        }
      },
      "required": [
        "default",
        "id",
        "value_type"
      ],
      "type": "object"
    },
    "ManifestSettingType": {
      "description": "Primitive setting value type.",
      "oneOf": [
        {
          "description": "UTF-8 text.",
          "enum": [
            "string"
          ],
          "type": "string"
        },
        {
          "description": "Boolean value.",
          "enum": [
            "boolean"
          ],
          "type": "string"
        },
        {
          "description": "Finite JSON number.",
          "enum": [
            "number"
          ],
          "type": "string"
        }
      ]
    },
    "ManifestTarget": {
      "additionalProperties": false,
      "description": "The single item an operation acts on. The host reads the item ID from the named input field, resolves its owning space with the existence-hiding resolver and authorizes the caller in that space before any plugin code runs.",
      "properties": {
        "field": {
          "description": "Top-level input property holding the item ID; the input schema must require it as a string.",
          "type": "string"
        },
        "kind": {
          "description": "Kind of the target item, such as `notes`; it may belong to another plugin.",
          "type": "string"
        }
      },
      "required": [
        "field",
        "kind"
      ],
      "type": "object"
    },
    "ManifestTheme": {
      "additionalProperties": false,
      "description": "Network-free theme asset; protected surfaces cannot be themed.",
      "properties": {
        "id": {
          "description": "Stable theme identifier.",
          "type": "string"
        },
        "stylesheet": {
          "description": "Relative packaged CSS asset; activation requires sealed theme validation.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "stylesheet"
      ],
      "type": "object"
    },
    "ManifestV1": {
      "additionalProperties": false,
      "description": "Plugin menu read without running plugin code. Every top-level field is required; only the documented optional fields inside declarations may be omitted.",
      "properties": {
        "api_version": {
          "description": "Manifest API version, currently 1.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "attention": {
          "description": "Attention sources.",
          "items": {
            "$ref": "#/definitions/ManifestAttention"
          },
          "type": "array"
        },
        "collections": {
          "description": "Collection provisioning templates.",
          "items": {
            "$ref": "#/definitions/ManifestCollection"
          },
          "type": "array"
        },
        "hooks": {
          "description": "Space-scoped watch or block hooks.",
          "items": {
            "$ref": "#/definitions/ManifestHook"
          },
          "type": "array"
        },
        "host_calls": {
          "description": "Broker calls the worker may request; each still needs live authorization.",
          "items": {
            "$ref": "#/definitions/HostCallKind"
          },
          "type": "array"
        },
        "icon": {
          "description": "Registered icon identifier.",
          "type": "string"
        },
        "id": {
          "description": "Stable plugin identifier.",
          "type": "string"
        },
        "kinds": {
          "description": "Kinds offered by this plugin.",
          "items": {
            "$ref": "#/definitions/ManifestKind"
          },
          "type": "array"
        },
        "name": {
          "description": "Human-readable plugin name.",
          "type": "string"
        },
        "operations": {
          "description": "Public operation schemas and policy.",
          "items": {
            "$ref": "#/definitions/ManifestOperation"
          },
          "type": "array"
        },
        "provider": {
          "anyOf": [
            {
              "$ref": "#/definitions/ManifestProvider"
            },
            {
              "type": "null"
            }
          ],
          "description": "Machine-side provider adapter; omitted by plugins that offer no provider."
        },
        "schedules": {
          "description": "Schedule declarations.",
          "items": {
            "$ref": "#/definitions/ManifestSchedule"
          },
          "type": "array"
        },
        "secrets": {
          "description": "Requested secret names only; no values.",
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "settings": {
          "description": "Typed space settings.",
          "items": {
            "$ref": "#/definitions/ManifestSetting"
          },
          "type": "array"
        },
        "themes": {
          "description": "Packaged theme declarations.",
          "items": {
            "$ref": "#/definitions/ManifestTheme"
          },
          "type": "array"
        },
        "version": {
          "description": "Exact package version.",
          "type": "string"
        },
        "views": {
          "description": "Shell region component references.",
          "items": {
            "$ref": "#/definitions/ManifestView"
          },
          "type": "array"
        },
        "webhooks": {
          "description": "Webhook declarations.",
          "items": {
            "$ref": "#/definitions/ManifestWebhook"
          },
          "type": "array"
        }
      },
      "required": [
        "api_version",
        "attention",
        "collections",
        "hooks",
        "host_calls",
        "icon",
        "id",
        "kinds",
        "name",
        "operations",
        "schedules",
        "secrets",
        "settings",
        "themes",
        "version",
        "views",
        "webhooks"
      ],
      "type": "object"
    },
    "ManifestView": {
      "additionalProperties": false,
      "description": "Component reference only; component format is separately versioned.",
      "properties": {
        "component": {
          "description": "Registered component identifier; no inline code.",
          "type": "string"
        },
        "id": {
          "description": "Stable contribution identifier.",
          "type": "string"
        },
        "region": {
          "allOf": [
            {
              "$ref": "#/definitions/ManifestRegion"
            }
          ],
          "description": "Shell region."
        }
      },
      "required": [
        "component",
        "id",
        "region"
      ],
      "type": "object"
    },
    "ManifestWebhook": {
      "additionalProperties": false,
      "description": "Webhook declaration; this does not mount an HTTP route.",
      "properties": {
        "id": {
          "description": "Stable webhook identifier.",
          "type": "string"
        },
        "operation": {
          "description": "Declared operation receiving authorized webhook input.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "operation"
      ],
      "type": "object"
    },
    "OperationRisk": {
      "description": "Declared risk class, evaluated in addition to capabilities and token scope.",
      "oneOf": [
        {
          "description": "Reads data without changing durable state.",
          "enum": [
            "read"
          ],
          "type": "string"
        },
        {
          "description": "Changes that ordinary application history can undo.",
          "enum": [
            "reversible"
          ],
          "type": "string"
        },
        {
          "description": "Effects that cannot reliably be undone; reviewer seam may advise here only.",
          "enum": [
            "irreversible"
          ],
          "type": "string"
        },
        {
          "description": "Communicates or publishes outside the space; approval is scoped.",
          "enum": [
            "external"
          ],
          "type": "string"
        },
        {
          "description": "Expands authority or changes access; always requires a person.",
          "enum": [
            "widens_access"
          ],
          "type": "string"
        }
      ]
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
              "$ref": "#/definitions/ManifestV1"
            }
          ],
          "description": "The plugin's full v1 manifest as registered; declarations grant no authority."
        }
      },
      "required": [
        "enabled",
        "generation",
        "manifest"
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
