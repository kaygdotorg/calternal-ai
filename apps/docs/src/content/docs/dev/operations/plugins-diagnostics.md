---
title: "plugins.diagnostics"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Inspect an installed plugin's readiness and recent worker failures

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "plugins.diagnostics",
  "description": "Inspect an installed plugin's readiness and recent worker failures",
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
  "definitions": {
    "PluginReadiness": {
      "description": "Last observed state of an installed package in one space.",
      "oneOf": [
        {
          "description": "Disabled in this space.",
          "enum": [
            "disabled"
          ],
          "type": "string"
        },
        {
          "description": "A manifest or bundle verification problem prevents execution.",
          "enum": [
            "unavailable"
          ],
          "type": "string"
        },
        {
          "description": "The enabled worker starts on its first invocation.",
          "enum": [
            "not_started"
          ],
          "type": "string"
        },
        {
          "description": "Native package or a worker whose last start completed.",
          "enum": [
            "ready"
          ],
          "type": "string"
        },
        {
          "description": "The last startup or running worker failed.",
          "enum": [
            "failed"
          ],
          "type": "string"
        }
      ]
    },
    "PluginRuntimeFailure": {
      "description": "One caller-safe runtime failure from this space.",
      "properties": {
        "at_ms": {
          "description": "When the host observed the failure, in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "operation": {
          "description": "Operation attempted, when the failure belongs to an invocation.",
          "type": [
            "string",
            "null"
          ]
        },
        "reason": {
          "description": "Stable reason code without private input or output values.",
          "type": "string"
        }
      },
      "required": [
        "at_ms",
        "reason"
      ],
      "type": "object"
    }
  },
  "description": "Installed-package diagnostics exposed to authorized space managers.",
  "properties": {
    "fetch_hosts": {
      "description": "Latest 32 distinct Class B hostnames attempted in this space over seven days.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "plugin_id": {
      "description": "Installed package ID.",
      "type": "string"
    },
    "readiness": {
      "allOf": [
        {
          "$ref": "#/definitions/PluginReadiness"
        }
      ],
      "description": "Last observed readiness in this space."
    },
    "reason": {
      "description": "Caller-safe reason when the package is unavailable or failed.",
      "type": [
        "string",
        "null"
      ]
    },
    "recent_errors": {
      "description": "Up to eight recent failures, newest first.",
      "items": {
        "$ref": "#/definitions/PluginRuntimeFailure"
      },
      "type": "array"
    },
    "revision": {
      "description": "Revision inspected by this read.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "space_id": {
      "description": "Explicit space UUID.",
      "type": "string"
    },
    "stderr_tail": {
      "description": "Up to 4 KiB from the last failed worker's own stderr in this space.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "fetch_hosts",
    "plugin_id",
    "readiness",
    "recent_errors",
    "revision",
    "space_id"
  ],
  "title": "PluginDiagnostics",
  "type": "object"
}
```
