---
title: "calternal search skills"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/search-skills.txt, crates/cli/tests/golden/search-skills.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `search skills --space UUID [PREFIX]` | List space skill names and descriptions. | Skill table. | `skills` |

## JSON data shape

```json
{
  "skills": [
    {
      "name": "string",
      "description": "string"
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
SKILL   DESCRIPTION
review  Review a draft.
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "skills": [
      {
        "description": "Review a draft.",
        "name": "review"
      }
    ]
  },
  "ok": true,
  "request_id": "req-search"
}
```
