---
title: "cl describe"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/describe.txt, crates/cli/tests/golden/describe.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `describe` | The server's protocol versions, your principal and spaces, and the operations it offers. | `protocol`, `genui`, `principal`; table `SPACE ROLE`; table `OPERATION ACCESS DESCRIPTION`. | the describe result: `protocol_version`, `genui_version`, `principal`, `operations[]` (with schemas) |

## JSON data shape

```json
{
  "protocol_version": "integer",
  "genui_version": "integer",
  "principal": {
    "id": "string",
    "spaces": [
      {
        "id": "string",
        "role": "string"
      }
    ]
  },
  "operations": [
    {
      "id": "string",
      "description": "string",
      "mutates": "bool",
      "input_schema": "any",
      "output_schema": "any"
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
protocol:   1
genui:      1
principal:  acct-1

SPACE    ROLE
space-1  owner

OPERATION   ACCESS  DESCRIPTION
notes.list  reads   List notes
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "genui_version": 1,
    "operations": [
      {
        "description": "List notes",
        "id": "notes.list",
        "input_schema": {},
        "mutates": false,
        "output_schema": {}
      }
    ],
    "principal": {
      "id": "acct-1",
      "spaces": [
        {
          "id": "space-1",
          "role": "owner"
        }
      ]
    },
    "protocol_version": 1
  },
  "ok": true
}
```
