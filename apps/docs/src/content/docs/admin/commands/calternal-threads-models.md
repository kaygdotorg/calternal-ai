---
title: "calternal threads models"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/threads-models-120.txt, crates/cli/tests/golden/threads-models.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `threads models --space-id SPACE_UUID` | Find selectable provider models | Model IDs and reasoning levels | `providers` |

## JSON data shape

```json
{
  "providers": [
    {
      "id": "string",
      "name": "string",
      "default_model": "string",
      "models": [
        {
          "id": "string",
          "name": "string",
          "reasoning_levels": [
            "string"
          ]
        }
      ]
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
Provider:     Claude Code
Provider ID:  calternal-claude-code
MODEL ID                   MODEL       REASONING                      DEFAULT
claude-sonnet-5-5          Sonnet 5.5  low, medium, high, xhigh, max  Yes
claude-haiku-4-5-20251001  Haiku 4.5   Provider default               No
Next: calternal threads send --help
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "providers": [
      {
        "default_model": "claude-sonnet-5-5",
        "id": "calternal-claude-code",
        "models": [
          {
            "id": "claude-sonnet-5-5",
            "name": "Sonnet 5.5",
            "reasoning_levels": [
              "low",
              "medium",
              "high",
              "xhigh",
              "max"
            ]
          },
          {
            "id": "claude-haiku-4-5-20251001",
            "name": "Haiku 4.5",
            "reasoning_levels": []
          }
        ],
        "name": "Claude Code"
      }
    ]
  },
  "ok": true
}
```
