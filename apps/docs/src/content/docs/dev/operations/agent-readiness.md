---
title: "agent.readiness"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read your machine and provider readiness in a space

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "agent.readiness",
  "description": "Read your machine and provider readiness in a space",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Select the space and optionally a saved thread's placement constraint.",
  "properties": {
    "space_id": {
      "description": "Space UUID. The person needs the space's plugins.use capability.",
      "type": "string"
    },
    "thread_id": {
      "default": null,
      "description": "Saved thread UUID, if readiness should be restricted by a thread pin.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "space_id"
  ],
  "title": "ReadinessInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "MachineExecution": {
      "description": "The effective execution environment available on a machine.",
      "oneOf": [
        {
          "description": "The machine runs turns in its configured sandbox.",
          "enum": [
            "sandboxed"
          ],
          "type": "string"
        },
        {
          "description": "The account chose execution without a container.",
          "enum": [
            "direct"
          ],
          "type": "string"
        }
      ]
    },
    "MachineReadiness": {
      "description": "An eligible personal machine or anonymous pool option.",
      "properties": {
        "available": {
          "description": "The machine is accepting additional placements.",
          "type": "boolean"
        },
        "connected": {
          "description": "The server has a current connection from the machine.",
          "type": "boolean"
        },
        "enrolled": {
          "description": "The machine has an active enrollment record.",
          "type": "boolean"
        },
        "execution": {
          "allOf": [
            {
              "$ref": "#/definitions/MachineExecution"
            }
          ],
          "description": "How this machine runs turns."
        },
        "providers": {
          "additionalProperties": {
            "$ref": "#/definitions/ProviderReadiness"
          },
          "description": "Installed provider plugins and their account-scoped check results.",
          "type": "object"
        },
        "sandbox_fallback": {
          "anyOf": [
            {
              "$ref": "#/definitions/SandboxFallback"
            },
            {
              "type": "null"
            }
          ],
          "description": "The cloud machine is using containers because KVM is unavailable."
        }
      },
      "required": [
        "available",
        "connected",
        "enrolled",
        "execution",
        "providers"
      ],
      "type": "object"
    },
    "PersonalMachineReadiness": {
      "description": "Readiness for a machine owned by the caller.",
      "properties": {
        "available": {
          "description": "The machine is accepting additional placements.",
          "type": "boolean"
        },
        "connected": {
          "description": "The server has a current connection from the machine.",
          "type": "boolean"
        },
        "enrolled": {
          "description": "The machine has an active enrollment record.",
          "type": "boolean"
        },
        "execution": {
          "allOf": [
            {
              "$ref": "#/definitions/MachineExecution"
            }
          ],
          "description": "How this machine runs turns."
        },
        "last_seen_ms": {
          "description": "Last observed contact in Unix milliseconds, when known.",
          "format": "int64",
          "type": [
            "integer",
            "null"
          ]
        },
        "machine_id": {
          "description": "Stable enrolled machine UUID.",
          "type": "string"
        },
        "name": {
          "description": "The owner's machine label.",
          "type": "string"
        },
        "providers": {
          "additionalProperties": {
            "$ref": "#/definitions/ProviderReadiness"
          },
          "description": "Installed provider plugins and their account-scoped check results.",
          "type": "object"
        },
        "sandbox_fallback": {
          "anyOf": [
            {
              "$ref": "#/definitions/SandboxFallback"
            },
            {
              "type": "null"
            }
          ],
          "description": "The cloud machine is using containers because KVM is unavailable."
        }
      },
      "required": [
        "available",
        "connected",
        "enrolled",
        "execution",
        "machine_id",
        "name",
        "providers"
      ],
      "type": "object"
    },
    "ProviderCheck": {
      "additionalProperties": false,
      "description": "A bounded observation of the official provider program's own check.",
      "properties": {
        "installed": {
          "description": "The provider program was found inside the machine's execution environment.",
          "type": "boolean"
        },
        "sign_in_command": {
          "description": "The program's own sign-in command, for display only.",
          "type": "string"
        },
        "signed_in": {
          "description": "The program reported an active login for this account.",
          "type": "boolean"
        }
      },
      "required": [
        "installed",
        "sign_in_command",
        "signed_in"
      ],
      "type": "object"
    },
    "ProviderReadiness": {
      "additionalProperties": false,
      "description": "A machine's readiness for one provider in this space.",
      "properties": {
        "check": {
          "anyOf": [
            {
              "$ref": "#/definitions/ProviderCheck"
            },
            {
              "type": "null"
            }
          ],
          "description": "Recent provider status, or null until a fresh check is available."
        },
        "enabled": {
          "description": "The provider and orchestrator are enabled for this space.",
          "type": "boolean"
        },
        "ready": {
          "description": "The machine has capacity and the enabled provider reports a login.",
          "type": "boolean"
        }
      },
      "required": [
        "enabled",
        "ready"
      ],
      "type": "object"
    },
    "SandboxFallback": {
      "description": "Why a machine is running a reduced sandbox configuration.",
      "oneOf": [
        {
          "description": "The current machine user cannot create KVM guests, so the role uses Podman.",
          "enum": [
            "kvm_unavailable"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "One cached readiness snapshot. Pool options carry no host identity.",
  "properties": {
    "check_ttl_ms": {
      "description": "Lifetime of one provider observation in milliseconds.",
      "format": "uint64",
      "minimum": 0,
      "type": "integer"
    },
    "machines": {
      "description": "Nonrevoked machines enrolled to the requesting account.",
      "items": {
        "$ref": "#/definitions/PersonalMachineReadiness"
      },
      "type": "array"
    },
    "pool": {
      "description": "Anonymous options from the entitled pool, if any.",
      "items": {
        "$ref": "#/definitions/MachineReadiness"
      },
      "type": "array"
    }
  },
  "required": [
    "check_ttl_ms",
    "machines",
    "pool"
  ],
  "title": "ReadinessResult",
  "type": "object"
}
```
