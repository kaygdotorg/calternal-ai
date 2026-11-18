---
title: "requests.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List requests waiting on the signed-in account

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "requests.list",
  "description": "List requests waiting on the signed-in account",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Account-private list input. No caller-supplied account is accepted.",
  "title": "AccountInputList",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "ApprovalLifetime": {
      "description": "How long approved authority remains usable.",
      "oneOf": [
        {
          "description": "The first matching action may use the grant.",
          "enum": [
            "once"
          ],
          "type": "string"
        },
        {
          "description": "The grant survives turns until the thread is archived or deleted.",
          "enum": [
            "this_thread"
          ],
          "type": "string"
        }
      ]
    },
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
    "PersonRequest": {
      "description": "A request is presented only to the authenticated recipient.",
      "properties": {
        "detail": {
          "allOf": [
            {
              "$ref": "#/definitions/RequestDetail"
            }
          ],
          "description": "Question, approval or trusted secret prompt."
        },
        "id": {
          "description": "Stable request UUID.",
          "type": "string"
        },
        "space_id": {
          "description": "Addressed space UUID, absent for personal secret requests.",
          "type": [
            "string",
            "null"
          ]
        },
        "thread_id": {
          "description": "Thread UUID when the request was raised during a turn.",
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "detail",
        "id"
      ],
      "type": "object"
    },
    "RequestDetail": {
      "description": "The information needed to render each pending request.",
      "oneOf": [
        {
          "description": "One generated form, which may have multiple fields.",
          "properties": {
            "form": {
              "allOf": [
                {
                  "$ref": "#/definitions/GenUiView"
                }
              ],
              "description": "Form drawn by the application."
            },
            "kind": {
              "enum": [
                "question"
              ],
              "type": "string"
            },
            "label": {
              "description": "Label shown in inbox summaries and non-interactive clients.",
              "type": "string"
            }
          },
          "required": [
            "form",
            "kind",
            "label"
          ],
          "type": "object"
        },
        {
          "description": "Trusted, fixed form describing the proposed authority.",
          "properties": {
            "action": {
              "additionalProperties": true,
              "description": "Exact operation details when raised by an automatic approval check.",
              "type": "object"
            },
            "app": {
              "description": "App operation prefix.",
              "type": "string"
            },
            "kind": {
              "enum": [
                "approval"
              ],
              "type": "string"
            },
            "lifetimes": {
              "description": "Choices enforced by the server for this action, in display order.",
              "items": {
                "$ref": "#/definitions/ApprovalLifetime"
              },
              "type": "array"
            },
            "write": {
              "description": "Whether the proposed grant includes writes.",
              "type": "boolean"
            }
          },
          "required": [
            "action",
            "app",
            "kind",
            "lifetimes",
            "write"
          ],
          "type": "object"
        },
        {
          "description": "Trusted, fixed form describing the existing secret request.",
          "properties": {
            "expires_at_ms": {
              "description": "Unix milliseconds when the original fifteen-minute request expires.",
              "format": "int64",
              "type": "integer"
            },
            "kind": {
              "enum": [
                "secret"
              ],
              "type": "string"
            },
            "label": {
              "description": "Short human-readable name.",
              "type": "string"
            },
            "name": {
              "description": "Secret name, never the value.",
              "type": "string"
            },
            "reason": {
              "description": "Why the caller requested the credential.",
              "type": "string"
            }
          },
          "required": [
            "expires_at_ms",
            "kind",
            "label",
            "name",
            "reason"
          ],
          "type": "object"
        }
      ]
    }
  },
  "description": "Account-private pending request snapshot.",
  "properties": {
    "requests": {
      "description": "Currently authorized requests, ordered by ID.",
      "items": {
        "$ref": "#/definitions/PersonRequest"
      },
      "type": "array"
    }
  },
  "required": [
    "requests"
  ],
  "title": "RequestsListResult",
  "type": "object"
}
```
