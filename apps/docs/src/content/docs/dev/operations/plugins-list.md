---
title: "plugins.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List registered plugins

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "plugins.list",
  "description": "List registered plugins",
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
          "description": "List a bounded page of live items from the calling plugin's own kind.",
          "enum": [
            "list_items_v1"
          ],
          "type": "string"
        },
        {
          "description": "Read the effective value of a setting declared by the calling package.",
          "enum": [
            "read_setting_v1"
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
        "searchable": {
          "description": "Opt in to indexing the title and Markdown body for space search.",
          "type": [
            "boolean",
            "null"
          ]
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
        "models": {
          "description": "Models offered by this adapter, validated and cached with the manifest.",
          "items": {
            "$ref": "#/definitions/ProviderModel"
          },
          "type": "array"
        },
        "protocol_version": {
          "description": "Version of the bounded stdio contract, currently one.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "sign_in": {
          "description": "Whether the executable implements the private `--sign-in` mode.",
          "type": "boolean"
        },
        "title_generation": {
          "description": "Whether the adapter implements the private, stateless `--title` command.",
          "type": "boolean"
        }
      },
      "required": [
        "default_model",
        "executable",
        "models",
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
          "default": [],
          "description": "Kinds offered by this plugin. An omitted list declares no kinds.",
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
        "owned_slots": {
          "description": "Up to 32 mounts owned by this package, resolved only while enabled.",
          "items": {
            "$ref": "#/definitions/OwnedPluginSlot"
          },
          "maxItems": 32,
          "type": [
            "array",
            "null"
          ]
        },
        "palette": {
          "anyOf": [
            {
              "$ref": "#/definitions/PaletteDeclaration"
            },
            {
              "type": "null"
            }
          ],
          "description": "Optional commands, extra phrases and Settings sections for the palette."
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
        "slots": {
          "description": "Optional contributions to host slots, validated without running code.",
          "items": {
            "$ref": "#/definitions/PluginSlotContribution"
          },
          "type": [
            "array",
            "null"
          ]
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
    "OwnedPluginSlot": {
      "additionalProperties": false,
      "description": "One package-owned slot, with explicit visibility, accepted renderers and capacity.",
      "properties": {
        "accepts": {
          "description": "Nonempty set of operation-backed renderer types. Duplicates are invalid.",
          "items": {
            "$ref": "#/definitions/OwnedPluginSlotKind"
          },
          "maxItems": 2,
          "minItems": 1,
          "type": "array"
        },
        "id": {
          "description": "Stable local name; the host obtains the owner ID from the enclosing manifest.",
          "type": "string"
        },
        "max_contributions": {
          "description": "Maximum visible contributions per listing, between 1 and 32.",
          "format": "uint8",
          "maximum": 32,
          "minimum": 1,
          "type": "integer"
        },
        "required_capability": {
          "allOf": [
            {
              "$ref": "#/definitions/ManifestCapability"
            }
          ],
          "description": "Space capability a caller must hold to resolve this slot."
        }
      },
      "required": [
        "accepts",
        "id",
        "max_contributions",
        "required_capability"
      ],
      "type": "object"
    },
    "OwnedPluginSlotId": {
      "additionalProperties": false,
      "description": "A package-qualified slot. Its namespace cannot address a host surface.",
      "properties": {
        "id": {
          "description": "Local slot ID in that package.",
          "type": "string"
        },
        "plugin_id": {
          "description": "Registered package that declares the slot.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "plugin_id"
      ],
      "type": "object"
    },
    "OwnedPluginSlotKind": {
      "description": "Operation-backed contribution types available to a package-owned mount.",
      "oneOf": [
        {
          "description": "A read-only operation yielding a portable generated view.",
          "enum": [
            "generated"
          ],
          "type": "string"
        },
        {
          "description": "An operation invoked through the host's normal command and approval path.",
          "enum": [
            "command"
          ],
          "type": "string"
        }
      ]
    },
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
    "PaletteDeclaration": {
      "additionalProperties": false,
      "description": "Optional palette declarations in a plugin manifest.",
      "properties": {
        "commands": {
          "description": "Commands with their extra search phrases.",
          "items": {
            "$ref": "#/definitions/PaletteCommand"
          },
          "type": "array"
        },
        "settings_sections": {
          "description": "Settings sections that the plugin contributes.",
          "items": {
            "$ref": "#/definitions/PaletteSettingsSection"
          },
          "type": "array"
        }
      },
      "required": [
        "commands",
        "settings_sections"
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
    },
    "PluginSlot": {
      "description": "Host-defined mount point for an activated plugin contribution.",
      "oneOf": [
        {
          "description": "A section in the single navigation sidebar.",
          "enum": [
            "sidebar_section"
          ],
          "type": "string"
        },
        {
          "description": "Content within a host-managed card or window.",
          "enum": [
            "card_window"
          ],
          "type": "string"
        },
        {
          "description": "Accessory row beside the host-owned composer.",
          "enum": [
            "composer_row"
          ],
          "type": "string"
        },
        {
          "description": "Generated decoration within a message row.",
          "enum": [
            "message_decoration"
          ],
          "type": "string"
        },
        {
          "description": "Bounded panel along a thread edge.",
          "enum": [
            "thread_edge"
          ],
          "type": "string"
        },
        {
          "description": "Section outside protected account controls.",
          "enum": [
            "settings_section"
          ],
          "type": "string"
        },
        {
          "description": "Command discoverable in the host action palette.",
          "enum": [
            "palette_commands"
          ],
          "type": "string"
        },
        {
          "description": "Host-timed notification content.",
          "enum": [
            "toasts"
          ],
          "type": "string"
        }
      ]
    },
    "PluginSlotContribution": {
      "additionalProperties": false,
      "description": "One plugin-owned contribution that the host can resolve without running code.",
      "properties": {
        "id": {
          "description": "Stable identifier unique within this manifest.",
          "type": "string"
        },
        "label": {
          "description": "Accessible name and visible label, bounded to 128 UTF-8 bytes.",
          "type": "string"
        },
        "renderer": {
          "allOf": [
            {
              "$ref": "#/definitions/PluginSlotRenderer"
            }
          ],
          "description": "Renderer or command to resolve."
        },
        "slot": {
          "allOf": [
            {
              "$ref": "#/definitions/PluginSlotTarget"
            }
          ],
          "description": "Extensible host mount or package-owned target, independent of the renderer."
        }
      },
      "required": [
        "id",
        "label",
        "renderer",
        "slot"
      ],
      "type": "object"
    },
    "PluginSlotRenderer": {
      "description": "A portable operation-backed contribution or a reference to a sealed web bundle.",
      "oneOf": [
        {
          "additionalProperties": false,
          "description": "Resolve through a declared read-only operation returning a generated view.",
          "properties": {
            "operation": {
              "description": "Plugin-local operation declaration to dispatch with current authority.",
              "type": "string"
            },
            "type": {
              "enum": [
                "generated"
              ],
              "type": "string"
            }
          },
          "required": [
            "operation",
            "type"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Advertise a packaged entry point for a sealed web renderer.",
          "properties": {
            "asset": {
              "description": "Relative HTML asset path. The prototype does not serve or mount this asset.",
              "type": "string"
            },
            "type": {
              "enum": [
                "sealed"
              ],
              "type": "string"
            }
          },
          "required": [
            "asset",
            "type"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "description": "Register a named palette action targeting a declared operation.",
          "properties": {
            "operation": {
              "description": "Operation to dispatch with current authority and normal approval checks.",
              "type": "string"
            },
            "type": {
              "enum": [
                "command"
              ],
              "type": "string"
            }
          },
          "required": [
            "operation",
            "type"
          ],
          "type": "object"
        }
      ]
    },
    "PluginSlotTarget": {
      "anyOf": [
        {
          "allOf": [
            {
              "$ref": "#/definitions/PluginSlot"
            }
          ],
          "description": "One extensible host mount point."
        },
        {
          "allOf": [
            {
              "$ref": "#/definitions/OwnedPluginSlotId"
            }
          ],
          "description": "A slot defined by an installed package."
        }
      ],
      "description": "Mount addressed either by the fixed host vocabulary or a package-qualified name."
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
    },
    "ProviderModel": {
      "additionalProperties": false,
      "description": "One selectable provider model and the explicit reasoning levels it accepts.",
      "properties": {
        "id": {
          "description": "Stable model identifier passed to the adapter.",
          "type": "string"
        },
        "name": {
          "description": "Short display name for the model picker.",
          "type": "string"
        },
        "reasoning_levels": {
          "description": "Ordered reasoning levels. Empty means the model accepts no explicit level.",
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "id",
        "name",
        "reasoning_levels"
      ],
      "type": "object"
    },
    "ResolvedOwnedPluginSlot": {
      "additionalProperties": false,
      "description": "An owned mount available in the caller's selected space.",
      "properties": {
        "definition": {
          "allOf": [
            {
              "$ref": "#/definitions/OwnedPluginSlot"
            }
          ],
          "description": "The validated declaration, including renderer and capacity constraints."
        },
        "generation": {
          "description": "Current owner activation generation, used to invalidate a mounted slot.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "plugin_id": {
          "description": "Owner package. Pair it with the local ID from the definition.",
          "type": "string"
        }
      },
      "required": [
        "definition",
        "generation",
        "plugin_id"
      ],
      "type": "object"
    },
    "ResolvedPluginSlot": {
      "additionalProperties": false,
      "description": "An available contribution resolved for the caller and selected space.",
      "properties": {
        "contribution": {
          "allOf": [
            {
              "$ref": "#/definitions/PluginSlotContribution"
            }
          ],
          "description": "Validated declaration; it conveys no authority to dispatch or mount code."
        },
        "generation": {
          "description": "Space activation generation to invalidate stale mounts.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "plugin_id": {
          "description": "Owning package identifier; pair with the local contribution ID.",
          "type": "string"
        }
      },
      "required": [
        "contribution",
        "generation",
        "plugin_id"
      ],
      "type": "object"
    }
  },
  "description": "Result for `plugins.list`, including registered manifests and activation state.",
  "properties": {
    "owned_slots": {
      "description": "Available owned mounts, ordered by owner package and local slot ID.",
      "items": {
        "$ref": "#/definitions/ResolvedOwnedPluginSlot"
      },
      "type": [
        "array",
        "null"
      ]
    },
    "plugins": {
      "description": "Registered plugins and their space-scoped activation state.",
      "items": {
        "$ref": "#/definitions/PluginState"
      },
      "type": "array"
    },
    "slots": {
      "description": "Available contributions in slot, package ID and local ID order.",
      "items": {
        "$ref": "#/definitions/ResolvedPluginSlot"
      },
      "type": [
        "array",
        "null"
      ]
    }
  },
  "required": [
    "plugins"
  ],
  "title": "PluginsListResult",
  "type": "object"
}
```
