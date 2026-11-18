---
title: "Plugin manifest schema"
---

<!-- Generated from: contract/plugin-manifest-v1.schema.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Plugin menu read without running plugin code. Every top-level field is required; only the documented optional fields inside declarations may be omitted.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `api_version` | yes | integer | Manifest API version, currently 1. |
| `attention` | yes | array | Attention sources. |
| `collections` | yes | array | Collection provisioning templates. |
| `hooks` | yes | array | Space-scoped watch or block hooks. |
| `host_calls` | yes | array | Broker calls the worker may request; each still needs live authorization. |
| `icon` | yes | string | Registered icon identifier. |
| `id` | yes | string | Stable plugin identifier. |
| `kinds` | yes | array | Kinds offered by this plugin. |
| `name` | yes | string | Human-readable plugin name. |
| `operations` | yes | array | Public operation schemas and policy. |
| `palette` | no | schema | Optional commands, extra phrases and Settings sections for the palette. |
| `provider` | no | schema | Machine-side provider adapter; omitted by plugins that offer no provider. |
| `schedules` | yes | array | Schedule declarations. |
| `secrets` | yes | array | Requested secret names only; no values. |
| `settings` | yes | array | Typed space settings. |
| `themes` | yes | array | Packaged theme declarations. |
| `version` | yes | string | Exact package version. |
| `views` | yes | array | Shell region component references. |
| `webhooks` | yes | array | Webhook declarations. |

## HostCallKind

The only broker methods a worker may request.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "The only broker methods a worker may request.",
  "oneOf": [
    {
      "description": "Read an authorized item.",
      "type": "string",
      "enum": [
        "read_item"
      ]
    },
    {
      "description": "Write an authorized item.",
      "type": "string",
      "enum": [
        "write_item"
      ]
    },
    {
      "description": "List an authorized collection.",
      "type": "string",
      "enum": [
        "list_collection"
      ]
    },
    {
      "description": "Emit an authorized event.",
      "type": "string",
      "enum": [
        "emit_event"
      ]
    },
    {
      "description": "Request durable job admission.",
      "type": "string",
      "enum": [
        "start_job"
      ]
    },
    {
      "description": "Durably record a running job's checkpoint, from its run invocation.",
      "type": "string",
      "enum": [
        "checkpoint_job"
      ]
    },
    {
      "description": "Request a scoped opaque handle, never secret bytes.",
      "type": "string",
      "enum": [
        "request_secret_handle"
      ]
    }
  ]
}
```

## ManifestAttention

Authorized attention candidates; the shell selects at most three.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Stable attention source identifier. |
| `operation` | yes | string | Declared operation providing candidates. |

```json
{
  "description": "Authorized attention candidates; the shell selects at most three.",
  "type": "object",
  "required": [
    "id",
    "operation"
  ],
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
  "additionalProperties": false
}
```

## ManifestCapability

Closed host vocabulary, matching protected host capability names.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "Closed host vocabulary, matching protected host capability names.",
  "oneOf": [
    {
      "description": "Read authorized space content.",
      "type": "string",
      "enum": [
        "space.read"
      ]
    },
    {
      "description": "Write authorized space content.",
      "type": "string",
      "enum": [
        "content.write"
      ]
    },
    {
      "description": "Use installed plugins.",
      "type": "string",
      "enum": [
        "plugins.use"
      ]
    },
    {
      "description": "Manage shared plugin installations.",
      "type": "string",
      "enum": [
        "plugins.manage"
      ]
    },
    {
      "description": "Invite space members.",
      "type": "string",
      "enum": [
        "members.invite"
      ]
    },
    {
      "description": "Manage ordinary space members.",
      "type": "string",
      "enum": [
        "members.manage"
      ]
    },
    {
      "description": "Manage Managers; owner constraints still apply.",
      "type": "string",
      "enum": [
        "managers.manage"
      ]
    },
    {
      "description": "Change shared settings.",
      "type": "string",
      "enum": [
        "space.settings"
      ]
    },
    {
      "description": "Explicitly transfer ownership.",
      "type": "string",
      "enum": [
        "space.transfer"
      ]
    },
    {
      "description": "Delete a space.",
      "type": "string",
      "enum": [
        "space.delete"
      ]
    },
    {
      "description": "List the acting account memberships.",
      "type": "string",
      "enum": [
        "spaces.list"
      ]
    },
    {
      "description": "Create a space for the acting account.",
      "type": "string",
      "enum": [
        "spaces.create"
      ]
    }
  ]
}
```

## ManifestCollection

Provisioning template; the host allocates a fresh Collection UUID per enable.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Globally unique template identifier, never a Collection address. |
| `kind` | yes | string | Kind of items in this collection. |
| `name` | yes | string | Display name for provisioned collections. |

```json
{
  "description": "Provisioning template; the host allocates a fresh Collection UUID per enable.",
  "type": "object",
  "required": [
    "id",
    "kind",
    "name"
  ],
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
  "additionalProperties": false
}
```

## ManifestHook

Space-scoped hook enabled by a Manager; never a global interceptor.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Stable hook identifier. |
| `mode` | yes | #/definitions/ManifestHookMode | Watch or block only. |
| `operation` | yes | string | Declared operation implementing the hook. |
| `patterns` | yes | array | Exact operation IDs or namespace patterns ending in .* . |

```json
{
  "description": "Space-scoped hook enabled by a Manager; never a global interceptor.",
  "type": "object",
  "required": [
    "id",
    "mode",
    "operation",
    "patterns"
  ],
  "properties": {
    "id": {
      "description": "Stable hook identifier.",
      "type": "string"
    },
    "mode": {
      "description": "Watch or block only.",
      "allOf": [
        {
          "$ref": "#/definitions/ManifestHookMode"
        }
      ]
    },
    "operation": {
      "description": "Declared operation implementing the hook.",
      "type": "string"
    },
    "patterns": {
      "description": "Exact operation IDs or namespace patterns ending in .* .",
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "additionalProperties": false
}
```

## ManifestHookMode

Hooks observe or block; rewriting is unavailable.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "Hooks observe or block; rewriting is unavailable.",
  "oneOf": [
    {
      "description": "Observe permitted matching operations.",
      "type": "string",
      "enum": [
        "watch"
      ]
    },
    {
      "description": "Return proceed, wait or reject under a host deadline.",
      "type": "string",
      "enum": [
        "block"
      ]
    }
  ]
}
```

## ManifestIdempotency

Retry behavior declared by an operation.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "Retry behavior declared by an operation.",
  "oneOf": [
    {
      "description": "Read with no durable effect.",
      "type": "string",
      "enum": [
        "read_only"
      ]
    },
    {
      "description": "Host receipt required before durable effects.",
      "type": "string",
      "enum": [
        "request_id"
      ]
    },
    {
      "description": "Automatic retry prohibited; reconcile uncertain outcomes.",
      "type": "string",
      "enum": [
        "never"
      ]
    }
  ]
}
```

## ManifestKind

Kind identity remains independent of plugin identity.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `claimed_text` | yes | boolean | True selects userdata-ssd for claimed text; false selects userdata-hdd for other files. |
| `extension` | no | string, null | Lowercase ASCII file extension without the dot (1 to 16 letters or digits). Claimed-text kinds are Markdown: absent or `md`. Other kinds must declare one; the host stores their bytes unparsed under it. |
| `id` | yes | string | Stable kind name used in item addresses. |
| `schema` | yes | schema | JSON Schema for the kind content; no executable code. |
| `searchable` | no | boolean, null | Opt in to indexing the title and Markdown body for space search. |
| `storage` | yes | #/definitions/ManifestPlacement | Placement within the selected per-space userdata tree; never an appdata database path. |

```json
{
  "description": "Kind identity remains independent of plugin identity.",
  "type": "object",
  "required": [
    "claimed_text",
    "id",
    "schema",
    "storage"
  ],
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
      "description": "Placement within the selected per-space userdata tree; never an appdata database path.",
      "allOf": [
        {
          "$ref": "#/definitions/ManifestPlacement"
        }
      ]
    }
  },
  "additionalProperties": false
}
```

## ManifestOperation

Public operation declaration consumed by host policy.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Globally unique operation identifier. |
| `idempotency` | yes | #/definitions/ManifestIdempotency | Retry and host receipt contract. |
| `input_schema` | yes | schema | JSON Schema of operation input. |
| `mutates` | yes | boolean | Whether this operation can produce durable effects. |
| `output_schema` | yes | schema | JSON Schema of successful output. |
| `required_capability` | yes | #/definitions/ManifestCapability | Required current host capability. |
| `risk` | yes | #/definitions/OperationRisk | Required risk declaration; capability checks still apply in every mode. |
| `target` | no | schema | Item the operation targets; absent (or null) for a space-scoped operation, whose wire input names its space in `space_id`. |

```json
{
  "description": "Public operation declaration consumed by host policy.",
  "type": "object",
  "required": [
    "id",
    "idempotency",
    "input_schema",
    "mutates",
    "output_schema",
    "required_capability",
    "risk"
  ],
  "properties": {
    "id": {
      "description": "Globally unique operation identifier.",
      "type": "string"
    },
    "idempotency": {
      "description": "Retry and host receipt contract.",
      "allOf": [
        {
          "$ref": "#/definitions/ManifestIdempotency"
        }
      ]
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
      "description": "Required current host capability.",
      "allOf": [
        {
          "$ref": "#/definitions/ManifestCapability"
        }
      ]
    },
    "risk": {
      "description": "Required risk declaration; capability checks still apply in every mode.",
      "allOf": [
        {
          "$ref": "#/definitions/OperationRisk"
        }
      ]
    },
    "target": {
      "description": "Item the operation targets; absent (or null) for a space-scoped operation, whose wire input names its space in `space_id`.",
      "anyOf": [
        {
          "$ref": "#/definitions/ManifestTarget"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "additionalProperties": false
}
```

## ManifestPlacement

Placement inside a space userdata tree; hidden files belong to the declaring plugin.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "Placement inside a space userdata tree; hidden files belong to the declaring plugin.",
  "anyOf": [
    {
      "description": "Visible subtree claimed by one plugin, with at most eight safe components.",
      "type": "object",
      "required": [
        "folder",
        "subdirectories"
      ],
      "properties": {
        "folder": {
          "description": "One safe visible top-level folder.",
          "type": "string"
        },
        "subdirectories": {
          "description": "Safe directory components below the folder; never a filesystem path.",
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      },
      "additionalProperties": false
    },
    {
      "description": "User-visible top-level folder; the host rejects cross-plugin folder claims.",
      "type": "object",
      "required": [
        "folder"
      ],
      "properties": {
        "folder": {
          "description": "One NFC folder component, with no leading dot or reserved name.",
          "type": "string"
        }
      },
      "additionalProperties": false
    },
    {
      "description": "Plugin-owned files ordinary people need not see.",
      "type": "object",
      "required": [
        "hidden"
      ],
      "properties": {
        "hidden": {
          "description": "Must be true; placement is `.calternal/<plugin-id>/<kind>/` within userdata.",
          "type": "boolean"
        }
      },
      "additionalProperties": false
    }
  ]
}
```

## ManifestProvider

Machine-side provider declaration; executable is installed in the sandbox image or account home, never downloaded or run by the server.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `default_model` | yes | string | Default installed-CLI model identifier, at most 128 UTF-8 bytes. |
| `executable` | yes | string | Basename resolved inside the sandbox's PATH; no shell, path or arguments. |
| `models` | yes | array | Models offered by this adapter, validated and cached with the manifest. |
| `protocol_version` | yes | integer | Version of the bounded stdio contract, currently one. |
| `sign_in` | no | boolean | Whether the executable implements the private `--sign-in` mode. |

```json
{
  "description": "Machine-side provider declaration; executable is installed in the sandbox image or account home, never downloaded or run by the server.",
  "type": "object",
  "required": [
    "default_model",
    "executable",
    "models",
    "protocol_version"
  ],
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
      "type": "array",
      "items": {
        "$ref": "#/definitions/ProviderModel"
      }
    },
    "protocol_version": {
      "description": "Version of the bounded stdio contract, currently one.",
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "sign_in": {
      "description": "Whether the executable implements the private `--sign-in` mode.",
      "type": "boolean"
    }
  },
  "additionalProperties": false
}
```

## ManifestRegion

Shell region populated by a component reference.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "Shell region populated by a component reference.",
  "oneOf": [
    {
      "description": "App sidebar body.",
      "type": "string",
      "enum": [
        "sidebar"
      ]
    },
    {
      "description": "Main card.",
      "type": "string",
      "enum": [
        "main"
      ]
    },
    {
      "description": "Panel in the right card.",
      "type": "string",
      "enum": [
        "right_card"
      ]
    }
  ]
}
```

## ManifestSchedule

Schedule declaration; durable execution belongs to the host.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `expression` | yes | string | Schedule expression; activation awaits host scheduler validation. |
| `id` | yes | string | Stable schedule identifier. |
| `operation` | yes | string | Declared operation to invoke. |

```json
{
  "description": "Schedule declaration; durable execution belongs to the host.",
  "type": "object",
  "required": [
    "expression",
    "id",
    "operation"
  ],
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
  "additionalProperties": false
}
```

## ManifestSetting

Typed space setting; secrets use separate handles.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `default` | yes | schema | Default setting value. |
| `id` | yes | string | Stable setting identifier. |
| `value_type` | yes | #/definitions/ManifestSettingType | Primitive type of the default and stored values. |

```json
{
  "description": "Typed space setting; secrets use separate handles.",
  "type": "object",
  "required": [
    "default",
    "id",
    "value_type"
  ],
  "properties": {
    "default": {
      "description": "Default setting value."
    },
    "id": {
      "description": "Stable setting identifier.",
      "type": "string"
    },
    "value_type": {
      "description": "Primitive type of the default and stored values.",
      "allOf": [
        {
          "$ref": "#/definitions/ManifestSettingType"
        }
      ]
    }
  },
  "additionalProperties": false
}
```

## ManifestSettingType

Primitive setting value type.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "Primitive setting value type.",
  "oneOf": [
    {
      "description": "UTF-8 text.",
      "type": "string",
      "enum": [
        "string"
      ]
    },
    {
      "description": "Boolean value.",
      "type": "string",
      "enum": [
        "boolean"
      ]
    },
    {
      "description": "Finite JSON number.",
      "type": "string",
      "enum": [
        "number"
      ]
    }
  ]
}
```

## ManifestTarget

The single item an operation acts on. The host reads the item ID from the named input field, resolves its owning space with the existence-hiding resolver and authorizes the caller in that space before any plugin code runs.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `field` | yes | string | Top-level input property holding the item ID; the input schema must require it as a string. |
| `kind` | yes | string | Kind of the target item, such as `notes`; it may belong to another plugin. |

```json
{
  "description": "The single item an operation acts on. The host reads the item ID from the named input field, resolves its owning space with the existence-hiding resolver and authorizes the caller in that space before any plugin code runs.",
  "type": "object",
  "required": [
    "field",
    "kind"
  ],
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
  "additionalProperties": false
}
```

## ManifestTheme

Network-free theme asset; protected surfaces cannot be themed.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Stable theme identifier. |
| `stylesheet` | yes | string | Relative packaged CSS asset; activation requires sealed theme validation. |

```json
{
  "description": "Network-free theme asset; protected surfaces cannot be themed.",
  "type": "object",
  "required": [
    "id",
    "stylesheet"
  ],
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
  "additionalProperties": false
}
```

## ManifestView

Component reference only; component format is separately versioned.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `component` | yes | string | Registered component identifier; no inline code. |
| `id` | yes | string | Stable contribution identifier. |
| `region` | yes | #/definitions/ManifestRegion | Shell region. |

```json
{
  "description": "Component reference only; component format is separately versioned.",
  "type": "object",
  "required": [
    "component",
    "id",
    "region"
  ],
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
      "description": "Shell region.",
      "allOf": [
        {
          "$ref": "#/definitions/ManifestRegion"
        }
      ]
    }
  },
  "additionalProperties": false
}
```

## ManifestWebhook

Webhook declaration; this does not mount an HTTP route.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Stable webhook identifier. |
| `operation` | yes | string | Declared operation receiving authorized webhook input. |

```json
{
  "description": "Webhook declaration; this does not mount an HTTP route.",
  "type": "object",
  "required": [
    "id",
    "operation"
  ],
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
  "additionalProperties": false
}
```

## OperationRisk

Declared risk class, evaluated in addition to capabilities and token scope.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |


```json
{
  "description": "Declared risk class, evaluated in addition to capabilities and token scope.",
  "oneOf": [
    {
      "description": "Reads data without changing durable state.",
      "type": "string",
      "enum": [
        "read"
      ]
    },
    {
      "description": "Changes that ordinary application history can undo.",
      "type": "string",
      "enum": [
        "reversible"
      ]
    },
    {
      "description": "Effects that cannot reliably be undone; reviewer seam may advise here only.",
      "type": "string",
      "enum": [
        "irreversible"
      ]
    },
    {
      "description": "Communicates or publishes outside the space; approval is scoped.",
      "type": "string",
      "enum": [
        "external"
      ]
    },
    {
      "description": "Expands authority or changes access; always requires a person.",
      "type": "string",
      "enum": [
        "widens_access"
      ]
    }
  ]
}
```

## PaletteCommand

One palette command, in the web palette's command shape.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `go` | no | string, null | Navigation address, if this command navigates. |
| `group` | yes | string | Display group. |
| `id` | yes | string | Stable, globally unique command identifier. |
| `label` | yes | string | Searchable command label. |
| `op` | no | string, null | Named host action, if the command invokes one. |
| `words` | no | array | Extra phrases that match this command. |

```json
{
  "description": "One palette command, in the web palette's command shape.",
  "type": "object",
  "required": [
    "group",
    "id",
    "label"
  ],
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
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "additionalProperties": false
}
```

## PaletteDeclaration

Optional palette declarations in a plugin manifest.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `commands` | yes | array | Commands with their extra search phrases. |
| `settings_sections` | yes | array | Settings sections that the plugin contributes. |

```json
{
  "description": "Optional palette declarations in a plugin manifest.",
  "type": "object",
  "required": [
    "commands",
    "settings_sections"
  ],
  "properties": {
    "commands": {
      "description": "Commands with their extra search phrases.",
      "type": "array",
      "items": {
        "$ref": "#/definitions/PaletteCommand"
      }
    },
    "settings_sections": {
      "description": "Settings sections that the plugin contributes.",
      "type": "array",
      "items": {
        "$ref": "#/definitions/PaletteSettingsSection"
      }
    }
  },
  "additionalProperties": false
}
```

## PaletteSettingsSection

A discoverable Settings section.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `go` | yes | string | Settings address. |
| `id` | yes | string | Stable section identifier. |
| `label` | yes | string | Section label. |

```json
{
  "description": "A discoverable Settings section.",
  "type": "object",
  "required": [
    "go",
    "id",
    "label"
  ],
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
  "additionalProperties": false
}
```

## ProviderModel

One selectable provider model and the explicit reasoning levels it accepts.

| Field | Required | Type / reference | Description |
| --- | --- | --- | --- |
| `id` | yes | string | Stable model identifier passed to the adapter. |
| `name` | yes | string | Short display name for the model picker. |
| `reasoning_levels` | yes | array | Ordered reasoning levels. Empty means the model accepts no explicit level. |

```json
{
  "description": "One selectable provider model and the explicit reasoning levels it accepts.",
  "type": "object",
  "required": [
    "id",
    "name",
    "reasoning_levels"
  ],
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
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "additionalProperties": false
}
```
